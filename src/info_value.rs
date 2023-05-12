use std::cell::RefCell;

use gtk::glib::{self, SignalHandlerId};
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use crate::pkgproperty::PkgProperty;

//------------------------------------------------------------------------------
// MODULE: INFOVALUE
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/info_value.ui")]
    pub struct InfoValue {
        #[template_child]
        pub image: TemplateChild<gtk::Image>,
        #[template_child]
        pub label: TemplateChild<gtk::Label>,

        pub bindings: RefCell<Vec<glib::Binding>>,
        pub signals: RefCell<Vec<glib::SignalHandlerId>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoValue {
        const NAME: &'static str = "InfoValue";
        type Type = super::InfoValue;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }
    
    impl ObjectImpl for InfoValue {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();
        }
    }

    impl WidgetImpl for InfoValue {}
    impl BoxImpl for InfoValue {}
    impl InfoValue {}
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoValue(ObjectSubclass<imp::InfoValue>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InfoValue {
    pub fn new() -> Self {
        glib::Object::builder()
            .build()
    }

    pub fn bind_properties(&self, property: &PkgProperty) {
        let imp = self.imp();

        let image = imp.image.get();
        let label = imp.label.get();

        let mut bindings = imp.bindings.borrow_mut();

        let binding = property.bind_property("icon", &image, "visible")
            .transform_to(|_, icon: Option<&str>| {
                let icon = icon.unwrap_or_default();
                Some(icon != "")
            })
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
        bindings.push(binding);

        let binding = property.bind_property("icon", &image, "icon-name")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
        bindings.push(binding);

        let binding = property.bind_property("value", &label, "label")
            .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
            .build();
        bindings.push(binding);
    }

    pub fn unbind_properties(&self) {
        for binding in self.imp().bindings.borrow_mut().drain(..) {
            binding.unbind();
        }
    }

    pub fn add_label_signal(&self, signal: SignalHandlerId) {
        let mut signals = self.imp().signals.borrow_mut();
        signals.push(signal);
    }

    pub fn drop_label_signals(&self) {
        for signal in self.imp().signals.borrow_mut().drain(..) {
            self.imp().label.disconnect(signal);
        }
    }
}
