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
        #[property(get, set = Self::set_current, default_value = gtk::INVALID_LIST_POSITION, construct)]
        current: Cell<u32>,
        #[property(get = Self::current_item, nullable)]
        current_item: PhantomData<Option<PkgObject>>,
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

        fn set_current(&self, index: u32) {
            if index < self.list.borrow().len() as u32 {
                self.current.set(index);
            } else {
                self.current.set(gtk::INVALID_LIST_POSITION);
            }

            let obj = self.obj();

            obj.notify_len();
            obj.notify_peek_previous();
            obj.notify_peek_next();
            obj.notify_current_item();
        }

        fn current_item(&self) -> Option<PkgObject> {
            let current = self.current.get();

            if current == gtk::INVALID_LIST_POSITION {
                None
            } else {
                self.list.borrow().get(current as usize).cloned()
            }
        }

        fn peek_previous(&self) -> bool {
            let current = self.current.get();

            current != gtk::INVALID_LIST_POSITION && current > 0
        }

        fn peek_next(&self) -> bool {
            let current = self.current.get();

            current.checked_add(1).filter(|&i| i < self.list.borrow().len() as u32).is_some()
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

        let current = item.map_or(gtk::INVALID_LIST_POSITION, |item| {
            list.push(item);

            0
        });

        drop(list);

        self.set_current(current);
    }

    pub fn move_previous(&self) {
        if self.peek_previous() {
            self.set_current(self.current() - 1);
        }
    }

    pub fn move_next(&self) {
        if self.peek_next() {
            self.set_current(self.current() + 1);
        }
    }

    pub fn set_current_or_make_last(&self, item: PkgObject) {
        let mut list = self.imp().list.borrow_mut();

        let current = list.iter().position(|pkg| pkg.name() == item.name()).map_or_else(|| {
            // If current item is not the last one, truncate the list
            let current = self.current();

            if let Some(i) = current.checked_add(1).filter(|&i| i < list.len() as u32) {
                list.truncate(i as usize);
            }

            // Append item and make current
            list.push(item);

            list.len() - 1
        }, |index| index);

        drop(list);

        self.set_current(current as u32);
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
