
use gtk::{gio, glib, pango};
use gtk::prelude::{ListModelExt, Cast, CastNone, IsA, ObjectExt};
use pango::{AttrList, Attribute};

//------------------------------------------------------------------------------
// TRAIT: ListStoreFind
//------------------------------------------------------------------------------
pub trait ListStoreFind {
    fn find_with<F, T>(&self, func: F) -> Option<T>
    where
        F: FnMut(&T) -> bool,
        T: IsA<glib::Object>;
}

impl ListStoreFind for gio::ListStore {
    //-----------------------------------
    // Find with function
    //-----------------------------------
    fn find_with<F, T>(&self, mut func: F) -> Option<T>
    where
        F: FnMut(&T) -> bool,
        T: IsA<glib::Object>
    {
        let index = self.find_with_equal_func(|obj| {
            let type_obj = obj.downcast_ref::<T>()
                .unwrap_or_else(|| panic!("Failed to downcast to '{}'", obj.type_()));

            func(type_obj)
        });

        index.and_then(|index| self.item(index).and_downcast::<T>())
    }
}

//------------------------------------------------------------------------------
// TRAIT: AttrListExt
//------------------------------------------------------------------------------
pub trait AttrListExt {
    fn add<T>(&self, attr: T, start: usize, end: usize)
    where
        T: Into<Attribute>;
}

impl AttrListExt for AttrList {
    //-----------------------------------
    // Add function
    //-----------------------------------
    fn add<T>(&self, attr: T, start: usize, end: usize)
    where
        T: Into<Attribute>
    {
        let mut base_attr: Attribute = attr.into();

        base_attr.set_start_index(start as u32);
        base_attr.set_end_index(end as u32);

        self.insert(base_attr);
    }
}
