/// Picker configuration options
#[derive(Debug, Clone)]
pub struct PickerOptions {
    /// Picker columns data (1 column for single picker, 2 columns for dual picker)
    pub columns: Vec<Vec<String>>,
    /// Text color for picker items
    pub text_color: String,
    /// Cancel button text
    pub cancel_text: String,
    /// Cancel button color
    pub cancel_color: String,
    /// Confirm button text (always required for pickers)
    pub confirm_text: String,
    /// Confirm button color (always required for pickers)
    pub confirm_color: String,
}

impl PickerOptions {
    /// Create a single-column picker
    ///
    /// # Arguments
    /// * `items` - List of items for the picker
    /// * `text_color` - Color for picker item text
    /// * `cancel_text` - Text for cancel button
    /// * `cancel_color` - Color for cancel button
    /// * `confirm_text` - Text for confirm button
    /// * `confirm_color` - Color for confirm button
    ///
    /// # Returns
    /// * `PickerOptions` - Configured picker options
    pub fn single_column(
        items: Vec<String>,
        text_color: String,
        cancel_text: String,
        cancel_color: String,
        confirm_text: String,
        confirm_color: String,
    ) -> Self {
        Self {
            columns: vec![items],
            text_color,
            cancel_text,
            cancel_color,
            confirm_text,
            confirm_color,
        }
    }

    /// Create a dual-column picker
    ///
    /// # Arguments
    /// * `first_column` - List of items for the first column
    /// * `second_column` - List of items for the second column
    /// * `text_color` - Color for picker item text
    /// * `cancel_text` - Text for cancel button
    /// * `cancel_color` - Color for cancel button
    /// * `confirm_text` - Text for confirm button
    /// * `confirm_color` - Color for confirm button
    ///
    /// # Returns
    /// * `PickerOptions` - Configured picker options
    pub fn dual_column(
        first_column: Vec<String>,
        second_column: Vec<String>,
        text_color: String,
        cancel_text: String,
        cancel_color: String,
        confirm_text: String,
        confirm_color: String,
    ) -> Self {
        Self {
            columns: vec![first_column, second_column],
            text_color,
            cancel_text,
            cancel_color,
            confirm_text,
            confirm_color,
        }
    }

    /// Check if the picker configuration is valid
    ///
    /// # Returns
    /// * `bool` - true if valid, false otherwise
    pub fn is_valid(&self) -> bool {
        // Must have 1 or 2 columns
        if self.columns.is_empty() || self.columns.len() > 2 {
            return false;
        }

        // Each column must have at least one item
        for column in &self.columns {
            if column.is_empty() {
                return false;
            }
        }

        // Confirm text and color are always required
        !self.confirm_text.is_empty() && !self.confirm_color.is_empty()
    }
}
