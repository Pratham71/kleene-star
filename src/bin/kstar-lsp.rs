//! `kstar-lsp`: the Kleene Star language server.
//!
//! An editor starts this program and talks to it over stdin and stdout using
//! the Language Server Protocol. All the language knowledge lives in
//! `kstar::ide`; this file only keeps the open documents and converts between
//! the protocol's line/character positions and byte offsets.

use std::collections::HashMap;
use std::sync::Mutex;
use kstar::ide::{self, Analysis, CompletionKind as Kind};
use kstar::lang::token::Span;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    /// The latest analysis of every open document.
    documents: Mutex<HashMap<Uri, Analysis>>,
}

fn range(src: &str, span: Span) -> Range {
    let (start_line, start_char) = ide::position_of(src, span.start);
    let (end_line, end_char) = ide::position_of(src, span.end);
    Range::new(Position::new(start_line, start_char), Position::new(end_line, end_char))
}

impl Backend {
    /// Analyses new text for `uri` and publishes its diagnostics.
    async fn update(&self, uri: Uri, text: String, version: Option<i32>) {
        let diagnostics = {
            let mut documents = self.documents.lock().expect("no thread panicked holding the lock");
            let analysis = Analysis::with_fallback(&text, documents.get(&uri));
            let diagnostics = analysis
                .problems
                .iter()
                .map(|problem| {
                    let mut message = problem.message.clone();
                    if let Some(hint) = &problem.hint {
                        message.push_str(&format!("\nhint: {hint}"));
                    }
                    let severity = if problem.warning { DiagnosticSeverity::WARNING } else { DiagnosticSeverity::ERROR };
                    Diagnostic {
                        range: range(&analysis.src, problem.span),
                        severity: Some(severity),
                        code: Some(NumberOrString::String(problem.code.into())),
                        source: Some("kstar".into()),
                        message,
                        ..Default::default()
                    }
                })
                .collect();
            documents.insert(uri.clone(), analysis);
            diagnostics
        };
        self.client.publish_diagnostics(uri, diagnostics, version).await;
    }

    /// Runs `f` on the analysis of the document at `position`, with the
    /// position as a byte offset.
    fn with_document<T>(&self, position: &TextDocumentPositionParams, f: impl FnOnce(&Analysis, usize) -> Option<T>) -> Option<T> {
        let documents = self.documents.lock().expect("no thread panicked holding the lock");
        let analysis = documents.get(&position.text_document.uri)?;
        let offset = ide::offset_at(&analysis.src, position.position.line, position.position.character);
        f(analysis, offset)
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo { name: "kstar-lsp".into(), version: Some(env!("CARGO_PKG_VERSION").into()) }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".into(), ",".into()]),
                    ..Default::default()
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            // Positions are in UTF-16 units, the protocol's default.
            offset_encoding: None,
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.update(doc.uri, doc.text, Some(doc.version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full sync: the last change holds the whole text.
        if let Some(change) = params.content_changes.into_iter().last() {
            let doc = params.text_document;
            self.update(doc.uri, change.text, Some(doc.version)).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.lock().expect("no thread panicked holding the lock").remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        Ok(self.with_document(&params.text_document_position_params, |a, offset| {
            let (span, text) = a.hover(offset)?;
            Some(Hover {
                contents: HoverContents::Markup(MarkupContent { kind: MarkupKind::Markdown, value: text }),
                range: Some(range(&a.src, span)),
            })
        }))
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let position = params.text_document_position_params;
        let uri = position.text_document.uri.clone();
        Ok(self.with_document(&position, |a, offset| {
            let span = a.definition(offset)?;
            Some(GotoDefinitionResponse::Scalar(Location::new(uri, range(&a.src, span))))
        }))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let position = params.text_document_position;
        let uri = position.text_document.uri.clone();
        let include = params.context.include_declaration;
        Ok(self.with_document(&position, |a, offset| {
            let spans = a.references(offset, include);
            Some(spans.into_iter().map(|span| Location::new(uri.clone(), range(&a.src, span))).collect())
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(self.with_document(&params.text_document_position, |a, offset| {
            let items = a
                .completions(offset)
                .into_iter()
                .map(|c| CompletionItem {
                    label: c.label,
                    kind: Some(match c.kind {
                        Kind::Keyword => CompletionItemKind::KEYWORD,
                        Kind::Function => CompletionItemKind::FUNCTION,
                        Kind::Variable => CompletionItemKind::VARIABLE,
                        Kind::State => CompletionItemKind::ENUM_MEMBER,
                        Kind::Symbol => CompletionItemKind::CONSTANT,
                    }),
                    detail: Some(c.detail),
                    documentation: c.doc.map(Documentation::String),
                    ..Default::default()
                })
                .collect();
            Some(CompletionResponse::Array(items))
        }))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let documents = self.documents.lock().expect("no thread panicked holding the lock");
        let Some(analysis) = documents.get(&params.text_document.uri) else { return Ok(None) };
        // One edit replacing the whole text: simple, and `kstar fmt` already
        // guarantees the meaning is unchanged.
        Ok(analysis.formatted().map(|text| {
            let end = ide::position_of(&analysis.src, analysis.src.len());
            vec![TextEdit::new(Range::new(Position::new(0, 0), Position::new(end.0, end.1)), text)]
        }))
    }

    async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>> {
        let documents = self.documents.lock().expect("no thread panicked holding the lock");
        let Some(analysis) = documents.get(&params.text_document.uri) else { return Ok(None) };
        if analysis.stale {
            return Ok(None);
        }
        let symbols = analysis.outline.iter().map(|s| document_symbol(&analysis.src, s)).collect();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        Ok(self.with_document(&params.text_document_position_params, |a, offset| {
            let help = a.signature_help(offset)?;
            Some(SignatureHelp {
                signatures: vec![SignatureInformation {
                    label: help.label,
                    documentation: help.doc.map(Documentation::String),
                    parameters: Some(
                        help.params
                            .into_iter()
                            .map(|p| ParameterInformation { label: ParameterLabel::Simple(p), documentation: None })
                            .collect(),
                    ),
                    active_parameter: None,
                }],
                active_signature: Some(0),
                active_parameter: Some(help.active as u32),
            })
        }))
    }
}

#[allow(deprecated)] // `deprecated` is a required field, but itself deprecated
fn document_symbol(src: &str, symbol: &ide::Symbol) -> DocumentSymbol {
    DocumentSymbol {
        name: symbol.name.clone(),
        detail: (!symbol.detail.is_empty()).then(|| symbol.detail.clone()),
        kind: match symbol.kind {
            ide::SymbolKind::Alphabet => SymbolKind::ENUM,
            ide::SymbolKind::Machine => SymbolKind::CLASS,
            ide::SymbolKind::Grammar => SymbolKind::STRUCT,
            ide::SymbolKind::Function => SymbolKind::FUNCTION,
            ide::SymbolKind::Value => SymbolKind::VARIABLE,
            ide::SymbolKind::Part => SymbolKind::ENUM_MEMBER,
        },
        tags: None,
        deprecated: None,
        range: range(src, symbol.span),
        selection_range: range(src, symbol.name_span),
        children: Some(symbol.children.iter().map(|c| document_symbol(src, c)).collect()),
    }
}

fn main() {
    // The parser, checker and formatter are recursive. The nesting limit
    // keeps them within a few megabytes even in a debug build, which is
    // more than a default 2 MB thread has, so give every thread plenty.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(kstar::STACK_SIZE)
        .enable_all()
        .build()
        .expect("could not start the language server");
    runtime.block_on(async {
        let (service, socket) =
            LspService::new(|client| Backend { client, documents: Mutex::new(HashMap::new()) });
        Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
    });
}
