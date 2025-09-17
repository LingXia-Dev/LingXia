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
    /// Confirm button text (optional, if None then only show cancel button)
    pub confirm_text: Option<String>,
    /// Confirm button color (required if confirm_text is Some)
    pub confirm_color: Option<String>,
}

impl PickerOptions {
    /// Create a single-column picker with only cancel button
    ///
    /// # Arguments
    /// * `items` - List of items for the picker
    /// * `text_color` - Color for picker item text
    /// * `cancel_text` - Text for cancel button
    /// * `cancel_color` - Color for cancel button
    ///
    /// # Returns
    /// * `PickerOptions` - Configured picker options
    pub fn single_column_cancel_only(
        items: Vec<String>,
        text_color: String,
        cancel_text: String,
        cancel_color: String,
    ) -> Self {
        Self {
            columns: vec![items],
            text_color,
            cancel_text,
            cancel_color,
            confirm_text: None,
            confirm_color: None,
        }
    }

    /// Create a single-column picker with cancel and confirm buttons
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
    pub fn single_column_with_confirm(
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
            confirm_text: Some(confirm_text),
            confirm_color: Some(confirm_color),
        }
    }

    /// Create a dual-column picker (always has both cancel and confirm buttons)
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
            confirm_text: Some(confirm_text),
            confirm_color: Some(confirm_color),
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

        // If confirm_text is Some, confirm_color must also be Some
        match (&self.confirm_text, &self.confirm_color) {
            (Some(_), Some(_)) => true,
            (None, None) => true,
            _ => false,
        }
    }
}
