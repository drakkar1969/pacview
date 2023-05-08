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
        pub searchtag_desc: TemplateChild<SearchTag>,
        #[template_child]
        pub searchtag_group: TemplateChild<SearchTag>,
        #[template_child]
        pub searchtag_deps: TemplateChild<SearchTag>,
        #[template_child]
        pub searchtag_optdeps: TemplateChild<SearchTag>,
        #[template_child]
        pub searchtag_provides: TemplateChild<SearchTag>,
        #[template_child]
        pub separator_exact: TemplateChild<gtk::Separator>,
        #[template_child]
        pub searchtag_exact: TemplateChild<SearchTag>,
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
        search_by_desc: Cell<bool>,
        #[property(get, set)]
        search_by_group: Cell<bool>,
        #[property(get, set)]
        search_by_deps: Cell<bool>,
        #[property(get, set)]
        search_by_optdeps: Cell<bool>,
        #[property(get, set)]
        search_by_provides: Cell<bool>,
        #[property(get, set)]
        search_exact: Cell<bool>,
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
        // Custom signals
        //-----------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("search-changed")
                    .param_types([
                        String::static_type(),
                        bool::static_type(),
                        bool::static_type(),
                        bool::static_type(),
                        bool::static_type(),
                        bool::static_type(),
                        bool::static_type(),
                        bool::static_type()])
                    .build(),
                    Signal::builder("search-activated")
                    .param_types([bool::static_type()])
                    .build(),
                ]
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

            // Bind title property to title widget
            obj.bind_property("title", &self.title_widget.get(), "title")
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            // Bind search by properties
            let tag_array = [
                self.searchtag_name.get(),
                self.searchtag_desc.get(),
                self.searchtag_group.get(),
                self.searchtag_deps.get(),
                self.searchtag_optdeps.get(),
                self.searchtag_provides.get(),
            ];

            for tag in tag_array {
                if let Some(text) = tag.text() {
                    let prop_name = format!("search-by-{}", text);

                    // Connect notify signals handlers for search by properties
                    obj.connect_notify(Some(&prop_name), move |header, _| {
                        let imp = header.imp();
        
                        header.emit_by_name::<()>("search-changed",
                            &[&imp.search_entry.text().to_string(),
                            &header.search_by_name(),
                            &header.search_by_desc(),
                            &header.search_by_group(),
                            &header.search_by_deps(),
                            &header.search_by_optdeps(),
                            &header.search_by_provides(),
                            &header.search_exact()]
                        );
                    });
    
                    // Bind search by properties to search tag visibility
                    obj.bind_property(&prop_name, &tag, "visible")
                        .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                        .build();
                }
            }

            // Connect notify signal handler for search exact property
            obj.connect_notify(Some("search-exact"), move |header, _| {
                let imp = header.imp();

                header.emit_by_name::<()>("search-changed",
                    &[&imp.search_entry.text().to_string(),
                    &header.search_by_name(),
                    &header.search_by_desc(),
                    &header.search_by_group(),
                    &header.search_by_deps(),
                    &header.search_by_optdeps(),
                    &header.search_by_provides(),
            &header.search_exact()]
                );
            });

            // Bind search exact property to search tag visibility
            obj.bind_property("search-exact", &self.separator_exact.get(), "visible")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();

            obj.bind_property("search-exact", &self.searchtag_exact.get(), "visible")
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();

            // Connect notify signal handler for search active property
            obj.connect_notify(Some("search-active"), |header, _| {
                let imp = header.imp();

                if header.search_active() {
                    imp.stack.set_visible_child_name("search");
    
                    imp.search_entry.grab_focus();
                } else {
                    imp.search_entry.set_text("");
    
                    imp.stack.set_visible_child_name("title");
                }

                header.emit_by_name::<()>("search-activated", &[&header.search_active()]);
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
        // Search entry signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_search_started(&self) {
            let obj = self.obj();

            obj.set_search_active(true);
        }

        #[template_callback]
        fn on_search_changed(&self) {
            let obj = self.obj();

            obj.emit_by_name::<()>("search-changed",
                &[&self.search_entry.text().to_string(),
                &obj.search_by_name(),
                &obj.search_by_desc(),
                &obj.search_by_group(),
                &obj.search_by_deps(),
                &obj.search_by_optdeps(),
                &obj.search_by_provides(),
                &obj.search_exact()]);
        }

        //-----------------------------------
        // Filter image signal handler
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
