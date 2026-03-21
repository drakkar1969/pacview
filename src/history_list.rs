use std::cell::{Cell, RefCell};
use std::marker::PhantomData;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: HistoryList
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::HistoryList)]
    pub struct HistoryList {
        #[property(get = Self::len)]
        len: PhantomData<u32>,
        #[property(get, set = Self::set_selected, default_value = gtk::INVALID_LIST_POSITION, construct)]
        selected: Cell<u32>,
        #[property(get = Self::selected_item, nullable)]
        selected_item: PhantomData<Option<PkgObject>>,
        #[property(get = Self::peek_previous)]
        peek_previous: PhantomData<bool>,
        #[property(get = Self::peek_next)]
        peek_next: PhantomData<bool>,

        pub(super) list: RefCell<Vec<PkgObject>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for HistoryList {
        const NAME: &'static str = "HistoryList";
        type Type = super::HistoryList;
    }

    #[glib::derived_properties]
    impl ObjectImpl for HistoryList {}

    impl HistoryList {
        //---------------------------------------
        // Property getters/setters
        //---------------------------------------
        fn len(&self) -> u32 {
            self.list.borrow().len() as u32
        }

        fn set_selected(&self, index: u32) {
            if index < self.list.borrow().len() as u32 {
                self.selected.set(index);
            } else {
                self.selected.set(gtk::INVALID_LIST_POSITION);
            }

            let obj = self.obj();

            obj.notify_len();
            obj.notify_peek_previous();
            obj.notify_peek_next();
            obj.notify_selected_item();
        }

        fn selected_item(&self) -> Option<PkgObject> {
            let selected = self.selected.get();

            if selected == gtk::INVALID_LIST_POSITION {
                None
            } else {
                self.list.borrow().get(selected as usize).cloned()
            }
        }

        fn peek_previous(&self) -> bool {
            let selected = self.selected.get();

            selected != gtk::INVALID_LIST_POSITION && selected > 0
        }

        fn peek_next(&self) -> bool {
            self.selected.get()
                .checked_add(1)
                .is_some_and(|i| i < self.list.borrow().len() as u32)
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: HistoryList
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct HistoryList(ObjectSubclass<imp::HistoryList>);
}

impl HistoryList {
    //---------------------------------------
    // Public functions
    //---------------------------------------
    pub fn init(&self, item: Option<PkgObject>) {
        let mut list = self.imp().list.borrow_mut();

        // Clear history and append item
        list.clear();

        let selected = item.map_or(gtk::INVALID_LIST_POSITION, |item| {
            list.push(item);

            0
        });

        drop(list);

        self.set_selected(selected);
    }

    pub fn select_previous(&self) {
        if self.peek_previous() {
            self.set_selected(self.selected() - 1);
        }
    }

    pub fn select_next(&self) {
        if self.peek_next() {
            self.set_selected(self.selected() + 1);
        }
    }

    pub fn select_or_append(&self, item: PkgObject) {
        let mut list = self.imp().list.borrow_mut();

        let selected = list.iter().position(|pkg| pkg.name() == item.name())
            .unwrap_or_else(|| {
                // If selected item is not the last one, truncate the list
                if let Some(i) = self.selected()
                    .checked_add(1)
                    .filter(|&i| i < list.len() as u32) {
                        list.truncate(i as usize);
                    }

                // Append item and select it
                list.push(item);

                list.len() - 1
            });

        drop(list);

        self.set_selected(selected as u32);
    }
}

impl Default for HistoryList {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
