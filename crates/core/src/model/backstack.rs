const TRIM_ABOVE: usize = 60;
const KEEP_AFTER_TRIM: usize = 30;

/// Back/forward navigation. Never empty: there is always a current place.
#[derive(Debug, Clone)]
pub struct Backstack<T> {
    places: Vec<T>,
    cursor: usize,
}

impl<T: PartialEq> Backstack<T> {
    pub fn new(start: T) -> Self {
        Self {
            places: vec![start],
            cursor: 0,
        }
    }

    pub fn current(&self) -> &T {
        &self.places[self.cursor]
    }

    /// Visits `place`, discarding the forward history; keeps the newest
    /// [`KEEP_AFTER_TRIM`] places once more than [`TRIM_ABOVE`] pile up.
    pub fn push(&mut self, place: T) {
        if *self.current() == place {
            return;
        }
        self.places.truncate(self.cursor + 1);
        self.places.push(place);
        if self.places.len() > TRIM_ABOVE {
            self.places.drain(..self.places.len() - KEEP_AFTER_TRIM);
        }
        self.cursor = self.places.len() - 1;
    }

    pub fn back(&mut self) -> Option<&T> {
        self.cursor = self.cursor.checked_sub(1)?;
        Some(self.current())
    }

    pub fn forward(&mut self) -> Option<&T> {
        if self.cursor + 1 >= self.places.len() {
            return None;
        }
        self.cursor += 1;
        Some(self.current())
    }

    pub fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.cursor + 1 < self.places.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_and_forward_walk_the_visits() {
        let mut stack = Backstack::new(0);
        stack.push(1);
        stack.push(2);
        assert_eq!(stack.back(), Some(&1));
        assert_eq!(stack.back(), Some(&0));
        assert_eq!(stack.back(), None);
        assert_eq!(stack.forward(), Some(&1));
        stack.push(9);
        assert!(!stack.can_go_forward());
        assert_eq!(stack.back(), Some(&1));
    }

    #[test]
    fn revisiting_the_current_place_is_not_a_visit() {
        let mut stack = Backstack::new(0);
        stack.push(0);
        assert!(!stack.can_go_back());
    }

    #[test]
    fn trims_to_the_newest_places_once_too_long() {
        let mut stack = Backstack::new(0);
        for place in 1..=TRIM_ABOVE {
            stack.push(place);
        }
        assert_eq!(stack.places.len(), KEEP_AFTER_TRIM);
        assert_eq!(*stack.current(), TRIM_ABOVE);
        assert_eq!(
            stack.places.first(),
            Some(&(TRIM_ABOVE + 1 - KEEP_AFTER_TRIM))
        );
        let mut steps = 0;
        while stack.back().is_some() {
            steps += 1;
        }
        assert_eq!(steps, KEEP_AFTER_TRIM - 1);
    }
}
