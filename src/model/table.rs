//! Table structures for document model.

use super::{Alignment, Paragraph};

/// A table in the document.
#[derive(Debug, Clone, Default)]
pub struct Table {
    /// Table rows
    pub rows: Vec<TableRow>,
    /// Column widths (optional, in percentage or pixels)
    pub column_widths: Vec<ColumnWidth>,
    /// Whether the first row is a header
    pub has_header: bool,
}

impl Table {
    /// Creates a new empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a table with the specified dimensions.
    pub fn with_dimensions(rows: usize, cols: usize) -> Self {
        let mut table = Self::new();
        for _ in 0..rows {
            let mut row = TableRow::new();
            for _ in 0..cols {
                row.cells.push(TableCell::new());
            }
            table.rows.push(row);
        }
        table
    }

    /// Returns the number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the number of columns (based on first row).
    pub fn column_count(&self) -> usize {
        self.rows.first().map(|r| r.cells.len()).unwrap_or(0)
    }

    /// Returns true if this table has any merged cells.
    pub fn has_merged_cells(&self) -> bool {
        self.rows
            .iter()
            .any(|row| row.cells.iter().any(|cell| cell.rowspan > 1 || cell.colspan > 1))
    }

    /// Gets a cell at the specified position.
    pub fn get_cell(&self, row: usize, col: usize) -> Option<&TableCell> {
        self.rows.get(row).and_then(|r| r.cells.get(col))
    }

    /// Gets a mutable cell at the specified position.
    pub fn get_cell_mut(&mut self, row: usize, col: usize) -> Option<&mut TableCell> {
        self.rows.get_mut(row).and_then(|r| r.cells.get_mut(col))
    }
}

/// A row in a table.
#[derive(Debug, Clone, Default)]
pub struct TableRow {
    /// Cells in this row
    pub cells: Vec<TableCell>,
    /// Whether this row is a header row
    pub is_header: bool,
}

impl TableRow {
    /// Creates a new empty row.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a header row.
    pub fn header() -> Self {
        Self {
            cells: Vec::new(),
            is_header: true,
        }
    }
}

/// A cell in a table.
#[derive(Debug, Clone, Default)]
pub struct TableCell {
    /// Content paragraphs within this cell
    pub content: Vec<Paragraph>,
    /// Number of rows this cell spans (default: 1)
    pub rowspan: u32,
    /// Number of columns this cell spans (default: 1)
    pub colspan: u32,
    /// Cell alignment
    pub alignment: Alignment,
    /// Vertical alignment
    pub vertical_alignment: VerticalAlignment,
    /// Background color (RGB hex)
    pub background_color: Option<String>,
}

impl TableCell {
    /// Creates a new empty cell.
    pub fn new() -> Self {
        Self {
            rowspan: 1,
            colspan: 1,
            ..Default::default()
        }
    }

    /// Creates a cell with text content.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![Paragraph::text(text)],
            rowspan: 1,
            colspan: 1,
            ..Default::default()
        }
    }

    /// Creates a cell with merged rows/columns.
    pub fn merged(rowspan: u32, colspan: u32) -> Self {
        Self {
            rowspan,
            colspan,
            ..Default::default()
        }
    }

    /// Returns the plain text content of this cell.
    pub fn plain_text(&self) -> String {
        self.content
            .iter()
            .map(|p| p.plain_text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Returns true if this cell is a merged cell placeholder.
    pub fn is_merged(&self) -> bool {
        self.rowspan > 1 || self.colspan > 1
    }
}

/// Vertical alignment options for table cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerticalAlignment {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Column width specification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnWidth {
    /// Automatic width
    Auto,
    /// Fixed width in pixels
    Pixels(u32),
    /// Percentage of table width
    Percent(f32),
}

impl Default for ColumnWidth {
    fn default() -> Self {
        Self::Auto
    }
}
