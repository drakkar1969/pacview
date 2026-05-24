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
        // Read/write property
        #[property(get = Self::selected, set = Self::set_selected, default_value = gtk::INVALID_LIST_POSITION, construct)]
        selected: PhantomData<u32>,

        // Read-only properties
        #[property(get = Self::len)]
        len: PhantomData<u32>,
        #[property(get = Self::selected_item, nullable)]
        selected_item: PhantomData<Option<PkgObject>>,
        #[property(get = Self::peek_previous)]
        peek_previous: PhantomData<bool>,
        #[property(get = Self::peek_next)]
        peek_next: PhantomData<bool>,

        // Internal fields
        pub(super) list: RefCell<Vec<PkgObject>>,
        pub(super) index: Cell<u32>,
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
        // Read/write property getter/setter
        //---------------------------------------
        fn selected(&self) -> u32 {
            self.index.get()
        }

        fn set_selected(&self, index: u32) {
            if index < self.list.borrow().len() as u32 {
                self.index.set(index);
            } else {
                self.index.set(gtk::INVALID_LIST_POSITION);
            }

            let obj = self.obj();

            obj.notify_len();
            obj.notify_peek_previous();
            obj.notify_peek_next();
            obj.notify_selected_item();
        }

        //---------------------------------------
        // Read-only property getters
        //---------------------------------------
        fn len(&self) -> u32 {
            self.list.borrow().len() as u32
        }

        fn selected_item(&self) -> Option<PkgObject> {
            let selected = self.selected();

            if selected == gtk::INVALID_LIST_POSITION {
                None
            } else {
                self.list.borrow().get(selected as usize).cloned()
            }
        }

        fn peek_previous(&self) -> bool {
            let selected = self.selected();

            selected != gtk::INVALID_LIST_POSITION && selected > 0
        }

        fn peek_next(&self) -> bool {
            self.selected()
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
        let imp = self.imp();

        let selected = if let Some(item) = item {
            imp.list.replace(vec![item]);

            0
        } else {
            imp.list.replace(vec![]);

            gtk::INVALID_LIST_POSITION
        };

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

    pub fn select_or_append(&self, new_item: PkgObject) {
        let selected = {
            let mut list = self.imp().list.borrow_mut();

            list.iter().position(|item| item == &new_item)
                .unwrap_or_else(|| {
                    // If selected item is not the last one, truncate the list
                    if let Some(i) = self.selected()
                        .checked_add(1)
                        .filter(|&i| i < list.len() as u32) {
                            list.truncate(i as usize);
                        }

                    // Append item and select it
                    list.push(new_item);

                    list.len() - 1
                })
        };

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
