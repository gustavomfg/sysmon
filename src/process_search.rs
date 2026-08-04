pub struct ProcessSearchState {
    open: bool,
    query: String,
    selected: usize,
}

impl ProcessSearchState {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected = 0;
    }

    pub fn insert(&mut self, character: char) {
        if !character.is_control() && self.query.chars().count() < 64 {
            self.query.push(character);
            self.selected = 0;
        }
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
    }

    pub fn move_selection(&mut self, direction: i32, count: usize) {
        if count == 0 {
            self.selected = 0;
            return;
        }

        self.selected = match direction.cmp(&0) {
            std::cmp::Ordering::Less if self.selected == 0 => count - 1,
            std::cmp::Ordering::Less => self.selected - 1,
            std::cmp::Ordering::Equal => self.selected,
            std::cmp::Ordering::Greater if self.selected + 1 == count => 0,
            std::cmp::Ordering::Greater => self.selected + 1,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::ProcessSearchState;

    #[test]
    fn supports_incremental_queries_and_wrapped_navigation() {
        let mut search = ProcessSearchState::new();
        search.open();
        search.insert('s');
        search.insert('h');
        assert_eq!(search.query(), "sh");

        search.move_selection(-1, 3);
        assert_eq!(search.selected_index(), 2);
        search.backspace();
        assert_eq!(search.query(), "s");
        assert_eq!(search.selected_index(), 0);
    }
}
