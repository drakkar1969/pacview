use std::cell::{Cell, RefCell};

use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// STRUCT: PkgHistory
//------------------------------------------------------------------------------
#[derive(Debug)]
pub struct PkgHistory {
    list: RefCell<Vec<PkgObject>>,
    index: Cell<Option<u32>>,
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: PkgHistory
//------------------------------------------------------------------------------
impl PkgHistory {
    //---------------------------------------
    // Public functions
    //---------------------------------------
    pub fn can_select_previous(&self) -> bool {
        self.index.get().is_some_and(|i| i > 0)
    }

    pub fn can_select_next(&self) -> bool {
        self.index.get().is_some_and(|i| i + 1 < self.list.borrow().len() as u32)
    }

    pub fn init(&self, item: Option<&PkgObject>) {
        let mut list = self.list.borrow_mut();

        list.clear();

        if let Some(item) = item {
            list.push(item.clone());

            self.index.replace(Some(0));
        } else {
            self.index.replace(None);
        }
    }

    pub fn len(&self) -> u32 {
        self.list.borrow().len() as u32
    }

    pub fn select_previous(&self) -> bool {
        let index = self.index.get();

        if self.can_select_previous() {
            self.index.set(index.map(|i| i - 1));

            true
        } else {
            false
        }
    }

    pub fn select_next(&self) -> bool {
        let index = self.index.get();

        if self.can_select_next() {
            self.index.set(index.map(|i| i + 1));

            true
        } else {
            false
        }
    }

    pub fn selected(&self) -> Option<u32> {
        self.index.get()
    }

    pub fn selected_item(&self) -> Option<PkgObject> {
        self.index.get()
            .and_then(|i| self.list.borrow().get(i as usize).cloned())
    }

    pub fn set_selected_item(&self, item: &PkgObject) -> bool {
        if let Some(index) = self.list.borrow().iter().position(|pkg| pkg.name() == item.name()) {
            self.index.set(Some(index as u32));

            true
        } else {
            false
        }
    }

    pub fn truncate_and_append(&self, item: &PkgObject) {
        let mut list = self.list.borrow_mut();

        if let Some(index) = self.index.get().filter(|&i| i + 1 < list.len() as u32) {
            list.truncate(index as usize + 1);
        }

        list.push(item.clone());
        self.index.replace(Some(list.len() as u32 - 1));
    }
}

impl Default for PkgHistory {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        Self {
            list: RefCell::new(vec![]),
            index: Cell::new(None)
        }
    }
}
