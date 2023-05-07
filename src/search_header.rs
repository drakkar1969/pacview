use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use glib::subclass::Signal;
use glib::once_cell::sync::Lazy;

use crate::search_tag::SearchTag;

//------------------------------------------------------------------------------
// MODULE: SEARCHHEADER
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SearchHeader)]
    #[template(resource = "/com/github/PacView/ui/search_header.ui")]
    pub struct SearchHeader {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub title_widget: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub searchtag_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub searchtag_name: TemplateChild<SearchTag>,
        #[template_child]
        pub searchtag_group: TemplateChild<SearchTag>,
        #[template_child]
        pub filter_popover: TemplateChild<gtk::PopoverMenu>,

        #[property(get, set)]
        title: RefCell<Option<String>>,
        #[property(get, set = Self::set_key_capture_widget)]
        key_capture_widget: RefCell<Option<gtk::Widget>>,
        #[property(get, set)]
        search_active: Cell<bool>,

        #[property(get, set)]
        search_by_name: Cell<bool>,
        #[property(get, set)]
        search_by_group: Cell<bool>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchHeader {
        const NAME: &'static str = "SearchHeader";
        type Type = super::SearchHeader;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            SearchTag::static_type();

            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }
    
    impl ObjectImpl for SearchHeader {
        //-----------------------------------
        // Custom signal
        //-----------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("search-changed")
                    .param_types([String::static_type()])
                    .build()]
            });
            SIGNALS.as_ref()
        }
    
        //-----------------------------------
        // Default property functions
        //-----------------------------------
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }
    
        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Position search tags
            if let Some(widget) = self.search_entry.get().first_child() {
                gtk::Widget::insert_after(&self.searchtag_box.get().upcast(), &self.search_entry.get(), Some(&widget));
            }

            // Set search by name active
            obj.set_search_by_name(true);

            // Bind title property to title widget
            obj.bind_property("title", &self.title_widget.get(), "title")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

            let tag_map = [
                self.searchtag_name.get(),
                self.searchtag_group.get(),
            ];

            // Bind search by properties
            for tag in tag_map {
                if let Some(text) = tag.text() {
                    let prop_name = format!("search-by-{}", text);

                    // Bind search by properties signal handlers
                    obj.connect_notify(Some(&prop_name), move |header, _| {
                        let imp = header.imp();
        
                        let search_text = imp.search_entry.text().to_string();
        
                        header.emit_by_name::<()>("search-changed", &[&search_text]);
                    });
    
                    // Bind search by properties to search tag visibility
                    obj.bind_property(&prop_name, &tag, "visible")
                    .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                    .build();
                }
            }

            // Bind search active property signal handler
            obj.connect_notify(Some("search-active"), |header, _| {
                let imp = header.imp();

                if header.search_active() {
                    imp.stack.set_visible_child_name("search");
    
                    imp.search_entry.grab_focus();
                } else {
                    imp.search_entry.set_text("");
    
                    imp.stack.set_visible_child_name("title");
    
                    if let Some(view) = header.key_capture_widget() {
                        view.grab_focus();
                    }
                }
            });
        }
    }

    impl WidgetImpl for SearchHeader {}
    impl BoxImpl for SearchHeader {}

    #[gtk::template_callbacks]
    impl SearchHeader {
        //-----------------------------------
        // Property getters/setters
        //-----------------------------------
        fn set_key_capture_widget(&self, widget: gtk::Widget) {
            self.search_entry.set_key_capture_widget(Some(&widget));

            *self.key_capture_widget.borrow_mut() = Some(widget);
        }

        //-----------------------------------
        // Search signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_search_started(&self) {
            let obj = self.obj();

            obj.set_search_active(true);
        }

        #[template_callback]
        fn on_search_changed(&self) {
            let obj = self.obj();

            obj.emit_by_name::<()>("search-changed", &[&self.search_entry.text().to_string()]);
        }

        //-----------------------------------
        // Filter signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_filter_image_clicked(&self) {
            self.filter_popover.popup();
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchHeader(ObjectSubclass<imp::SearchHeader>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SearchHeader {
    pub fn new() -> Self {
        glib::Object::builder()
            .build()
    }
}
