//! Embedded Typst PDF report renderer.

use std::path::Path;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Duration, Str, Value};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};

use crate::xlsx::{hex_sha256, publish_new_file};
use crate::{ReportDocument, ReportingError};

/// Fixed trusted template. Report data is supplied only through
/// `sys.inputs` as JSON, so workbook-controlled strings are never parsed as
/// Typst source code.
const PDF_TEMPLATE: &str = r##"
#let data = json(bytes(sys.inputs.at("report")))
#let l = data.labels
#let m = data.metadata
#let direction = if data.right_to_left { rtl } else { ltr }
#let text_align = if data.right_to_left { right } else { left }

#set page(
  paper: "a4",
  margin: (x: 15mm, y: 14mm),
  footer: context [
    #align(center)[#counter(page).display("1 / 1")]
  ],
)
#set document(title: l.report_title, author: "OpenConKit", date: none)
#set text(
  font: "DejaVu Sans Mono",
  size: 8.5pt,
  lang: m.language,
  dir: direction,
)
#set par(justify: true, leading: 0.7em)

#let heading(title) = [
  #v(8pt)
  #text(15pt, weight: "bold", fill: rgb("#176B68"))[#title]
  #line(length: 100%, stroke: 0.7pt + rgb("#8ABDB8"))
  #v(4pt)
]

#let field(label, value) = grid(
  columns: (32%, 68%),
  gutter: 6pt,
  inset: 4pt,
  stroke: 0.35pt + rgb("#C9D9D7"),
  fill: (rgb("#EAF4F2"), white),
  [#strong(label)],
  [#value],
)

#align(text_align)[
  #block(
    width: 100%,
    inset: 12pt,
    radius: 5pt,
    fill: rgb("#176B68"),
  )[
    #text(22pt, weight: "bold", fill: white)[#l.report_title]
    #v(3pt)
    #text(11pt, fill: rgb("#D7EFEC"))[#m.tool_name]
  ]

  #v(10pt)
  #field(l.source_file, m.source_filename)
  #field(l.source_hash, m.source_sha256)
  #field(l.run, m.run_id)
  #field(l.tool_version, m.tool_version)
  #field(l.rule_set_version, m.rule_set_version)
  #field(l.app_version, m.app_version)
  #field(l.report_timestamp, m.report_timestamp)
  #field(l.language, m.language)

  #heading(l.executive_summary)
  #grid(
    columns: (1fr, 1fr, 1fr),
    gutter: 6pt,
    block(inset: 8pt, radius: 4pt, fill: rgb("#EAF4F2"))[
      #text(18pt, weight: "bold")[#data.summary.item_rows]
      #linebreak()
      #l.item_rows
    ],
    block(inset: 8pt, radius: 4pt, fill: rgb("#EAF4F2"))[
      #text(18pt, weight: "bold")[#data.summary.finding_count]
      #linebreak()
      #l.finding_count
    ],
    block(inset: 8pt, radius: 4pt, fill: rgb("#EAF4F2"))[
      #text(18pt, weight: "bold")[
        #calc.round(data.summary.interpretation_confidence * 100, digits: 1)%
      ]
      #linebreak()
      #l.interpretation_confidence
    ],
  )

  #v(6pt)
  #grid(
    columns: (1fr, 1fr),
    gutter: 8pt,
    block(inset: 6pt, stroke: 0.4pt + rgb("#C9D9D7"))[
      #strong(l.severity)
      #for item in data.summary.severity_counts {
        [#linebreak()#item.at(0);: #item.at(1)]
      }
    ],
    block(inset: 6pt, stroke: 0.4pt + rgb("#C9D9D7"))[
      #strong(l.category)
      #for item in data.summary.category_counts {
        [#linebreak()#item.at(0);: #item.at(1)]
      }
    ],
  )

  #heading(l.findings)
  #if data.findings.len() == 0 [
    #l.not_available
  ]
  #for finding in data.findings {
    block(
      width: 100%,
      breakable: false,
      inset: 7pt,
      radius: 4pt,
      stroke: 0.5pt + rgb("#8ABDB8"),
    )[
      #grid(
        columns: (1fr, 1fr, 1fr),
        gutter: 5pt,
        [#strong(l.severity);: #finding.severity],
        [#strong(l.category);: #finding.category],
        [#strong(l.confidence);: #finding.confidence_percent%],
      )
      #v(3pt)
      #text(10pt, weight: "bold")[#finding.title]
      #linebreak()
      #finding.explanation
      #if finding.action != none [
        #linebreak()
        #strong(l.action);: #finding.action
      ]
      #if finding.sheet != none [
        #linebreak()
        #strong(l.sheet);: #finding.sheet
      ]
      #if finding.evidence.len() > 0 [
        #linebreak()
        #strong(l.evidence)
        #for ev in finding.evidence {
          [#linebreak()- #(ev.sheet) / #(ev.reference);: #(ev.description)]
        }
      ]
    ]
    v(5pt)
  }

  #heading(l.detection)
  #if data.detections.len() == 0 [
    #l.not_available
  ]
  #for detection in data.detections {
    block(width: 100%, inset: 6pt, stroke: 0.4pt + rgb("#C9D9D7"))[
      #strong(detection.sheet) / #detection.table_range
      #linebreak()
      #l.confidence;: #detection.confidence_percent%
      #linebreak()
      #l.mapped_columns;: #detection.mapped_columns
      #linebreak()
      #l.evidence;: #detection.evidence
      #if detection.warning != none [
        #linebreak()
        #strong(l.warning);: #detection.warning
      ]
    ]
    v(4pt)
  }

  #if data.pareto.len() > 0 [
    #heading(l.pareto)
    #for item in data.pareto {
      block(width: 100%, inset: 6pt, stroke: 0.4pt + rgb("#C9D9D7"))[
        #strong(l.context);: #item.context
        #linebreak()
        #l.currency;: #if item.currency == none { l.not_available } else { item.currency }
        #linebreak()
        #l.total_amount;: #item.total_amount
        #linebreak()
        #l.top_item_count;: #item.top_item_count / #item.total_item_count
        #linebreak()
        #l.cumulative_share;: #item.cumulative_share_percent%
      ]
      v(4pt)
    }
  ]

  #heading(l.limitations)
  #for limitation in data.limitations {
    [- #limitation]
  }

  #if data.ai_commentary != none [
    #heading(l.ai_review)
    #block(width: 100%, inset: 8pt, fill: rgb("#F4F0FA"))[
      #strong(l.ai_review)
      #linebreak()
      #data.ai_commentary
    ]
  ]
]
"##;

/// Render and atomically publish a new embedded-font PDF report.
///
/// Returns the lowercase SHA-256 of the generated artifact. Existing
/// destinations are never replaced.
pub fn write_pdf_report(path: &Path, report: &ReportDocument) -> Result<String, ReportingError> {
    report.validate()?;
    let json = serde_json::to_string(report)
        .map_err(|error| ReportingError::InvalidData(error.to_string()))?;
    let world = ReportWorld::new(json)?;
    let compiled = typst::compile(&world);
    let document = compiled.output.map_err(|diagnostics| {
        let messages = diagnostics
            .iter()
            .map(|diagnostic| {
                let line = world
                    .range(diagnostic.span)
                    .map(|range| PDF_TEMPLATE[..range.start].lines().count())
                    .unwrap_or(0);
                format!("line {line}: {}", diagnostic.message)
            })
            .collect::<Vec<_>>()
            .join("; ");
        ReportingError::Pdf(format!(
            "trusted template compilation produced {} error(s): {messages}",
            diagnostics.len()
        ))
    })?;
    let bytes =
        typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|diagnostics| {
            ReportingError::Pdf(format!(
                "PDF encoding produced {} error(s)",
                diagnostics.len()
            ))
        })?;
    publish_new_file(path, &bytes)?;
    Ok(hex_sha256(&bytes))
}

struct ReportWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    source: Source,
}

impl ReportWorld {
    fn new(report_json: String) -> Result<Self, ReportingError> {
        let mut inputs = Dict::new();
        inputs.insert(Str::from("report"), Value::Str(Str::from(report_json)));
        let library = LazyHash::new(Library::builder().with_inputs(inputs).build());
        let fonts = typst_assets::fonts()
            .flat_map(|bytes| Font::iter(Bytes::new(bytes)))
            .collect::<Vec<_>>();
        if fonts.is_empty() {
            return Err(ReportingError::Pdf(
                "embedded Typst fonts are unavailable".to_string(),
            ));
        }
        let book = LazyHash::new(FontBook::from_fonts(&fonts));
        let source = Source::detached(PDF_TEMPLATE);
        Ok(Self {
            library,
            book,
            fonts,
            source,
        })
    }
}

impl World for ReportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.source.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.source.id() {
            Ok(Bytes::from_string(self.source.text().to_string()))
        } else {
            Err(FileError::AccessDenied)
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use hayro_interpret::font::Glyph;
    use hayro_interpret::hayro_cmap::BfString;
    use hayro_interpret::hayro_syntax::Pdf;
    use hayro_interpret::{
        interpret_page, BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image,
        InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask,
    };
    use kurbo::{Affine, BezPath, Rect};

    use super::*;

    #[derive(Default)]
    struct UnicodeTextExtractor {
        text: String,
    }

    impl Device<'_> for UnicodeTextExtractor {
        fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}

        fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {}

        fn push_clip_path(&mut self, _: &ClipPath) {}

        fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}

        fn draw_glyph(
            &mut self,
            glyph: &Glyph<'_>,
            _: Affine,
            _: Affine,
            _: &Paint<'_>,
            _: &GlyphDrawMode,
        ) {
            match glyph.as_unicode() {
                Some(BfString::Char(character)) => self.text.push(character),
                Some(BfString::String(text)) => self.text.push_str(&text),
                None => self.text.push('\u{FFFD}'),
            }
        }

        fn pop_clip_path(&mut self) {}

        fn pop_transparency_group(&mut self) {}

        fn draw_image(&mut self, _: Image<'_, '_>, _: Affine) {}

        fn set_blend_mode(&mut self, _: BlendMode) {}
    }

    fn extract_unicode_text(bytes: &[u8]) -> String {
        let pdf = Pdf::new(bytes.to_vec()).expect("parses generated PDF");
        let cache = InterpreterCache::new();
        let mut extractor = UnicodeTextExtractor::default();
        for page in pdf.pages().iter() {
            let mut context = Context::new(
                Affine::IDENTITY,
                Rect::new(0.0, 0.0, 1.0, 1.0),
                &cache,
                pdf.xref(),
                InterpreterSettings::default(),
            );
            interpret_page(page, &mut context, &mut extractor);
        }
        extractor.text
    }

    fn contains_logical_or_visual_rtl(extracted: &str, expected: &str) -> bool {
        extracted.contains(expected)
            || extracted.contains(&expected.chars().rev().collect::<String>())
    }

    fn is_arabic_scalar(character: char) -> bool {
        matches!(
            character,
            '\u{0600}'..='\u{06FF}'
                | '\u{0750}'..='\u{077F}'
                | '\u{0870}'..='\u{089F}'
                | '\u{08A0}'..='\u{08FF}'
                | '\u{FB50}'..='\u{FDFF}'
                | '\u{FE70}'..='\u{FEFF}'
        )
    }

    fn preview_path(filename: &str) -> (std::path::PathBuf, bool) {
        if std::env::var("OPENCONKIT_KEEP_REPORT_PREVIEWS").as_deref() == Ok("1") {
            (
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("output")
                    .join("pdf")
                    .join(filename),
                true,
            )
        } else {
            (
                std::env::temp_dir().join(format!(
                    "openconkit-reporting-{filename}-{}",
                    std::process::id()
                )),
                false,
            )
        }
    }

    #[test]
    fn writes_embedded_font_english_and_arabic_pdfs_and_refuses_overwrite() {
        let (path, keep) = preview_path("boq-report-en.pdf");
        let _ = std::fs::remove_file(&path);
        let report = crate::xlsx::tests::sample_report();
        let hash = write_pdf_report(&path, &report).expect("writes PDF");
        assert_eq!(hash.len(), 64);
        let bytes = std::fs::read(&path).expect("reads PDF");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 10_000);
        let english_text = extract_unicode_text(&bytes);
        assert!(english_text.contains(&report.labels.report_title));
        assert!(english_text.contains(&report.labels.findings));
        assert!(matches!(
            write_pdf_report(&path, &report),
            Err(ReportingError::AlreadyExists(_))
        ));

        let (arabic_path, keep_arabic) = preview_path("boq-report-ar.pdf");
        let _ = std::fs::remove_file(&arabic_path);
        let mut arabic = report;
        arabic.metadata.language = "ar".into();
        arabic.right_to_left = true;
        arabic.labels.report_title = "تقرير مدقق جداول الكميات".into();
        arabic.labels.executive_summary = "الملخص التنفيذي".into();
        arabic.labels.findings = "النتائج".into();
        arabic.labels.limitations = "القيود".into();
        arabic.findings[0].title = "المبلغ لا يساوي الكمية في السعر".into();
        arabic.findings[0].explanation = "القيمة المتوقعة 10 والقيمة الموجودة 11.".into();
        arabic.limitations = vec!["لا تُقيَّم الصيغ غير المدعومة، بل تُعرض للمراجعة اليدوية.".into()];
        let arabic_hash = write_pdf_report(&arabic_path, &arabic).expect("writes Arabic PDF");
        assert_eq!(arabic_hash.len(), 64);
        let arabic_bytes = std::fs::read(&arabic_path).expect("reads Arabic PDF");
        assert!(arabic_bytes.starts_with(b"%PDF-"));
        assert!(arabic_bytes.len() > 10_000);
        let arabic_text = extract_unicode_text(&arabic_bytes);
        assert!(
            contains_logical_or_visual_rtl(&arabic_text, &arabic.labels.report_title),
            "extracted Arabic text: {arabic_text:?}"
        );
        assert!(contains_logical_or_visual_rtl(
            &arabic_text,
            &arabic.labels.findings
        ));
        assert!(
            arabic_text
                .chars()
                .filter(|value| is_arabic_scalar(*value))
                .count()
                >= 50,
            "expected substantial extractable Arabic Unicode text"
        );

        if !keep {
            std::fs::remove_file(&path).ok();
        }
        if !keep_arabic {
            std::fs::remove_file(&arabic_path).ok();
        }
    }

    #[test]
    fn template_accepts_data_only_through_inputs() {
        assert!(!PDF_TEMPLATE.contains("{{"));
        assert!(PDF_TEMPLATE.contains("sys.inputs.at(\"report\")"));
    }
}
