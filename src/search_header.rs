use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::*;

use glib::subclass::Signal;
use glib::once_cell::sync::Lazy;

use crate::search_tag::SearchTag;

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchMode")]
pub enum SearchMode {
    #[enum_value(name = "All")]
    All = 0,
    #[enum_value(name = "Any")]
    Any = 1,
    #[enum_value(name = "Exact")]
    Exact = 2,
}

impl Default for SearchMode {
    fn default() -> Self {
        SearchMode::All
    }
}

//------------------------------------------------------------------------------
// MODULE: SearchHeader
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
        pub searchtag_files: TemplateChild<SearchTag>,
        #[template_child]

        pub searchtag_all: TemplateChild<SearchTag>,
        #[template_child]
        pub searchtag_any: TemplateChild<SearchTag>,
        #[template_child]
        pub searchtag_exact: TemplateChild<SearchTag>,
        #[template_child]

        pub filter_popover: TemplateChild<gtk::PopoverMenu>,

        #[property(get, set)]
        title: RefCell<Option<String>>,
        #[property(get, set = Self::set_key_capture_widget)]
        key_capture_widget: RefCell<Option<gtk::Widget>>,

        #[property(get, set)]
        active: Cell<bool>,

        #[property(get, set)]
        by_name: Cell<bool>,
        #[property(get, set)]
        by_desc: Cell<bool>,
        #[property(get, set)]
        by_group: Cell<bool>,
        #[property(get, set)]
        by_deps: Cell<bool>,
        #[property(get, set)]
        by_optdeps: Cell<bool>,
        #[property(get, set)]
        by_provides: Cell<bool>,
        #[property(get, set)]
        by_files: Cell<bool>,

        #[property(get, set, builder(SearchMode::default()))]
        mode: Cell<SearchMode>,

        #[property(get, set)]
        block_notify: Cell<bool>,
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
            SearchMode::static_type();

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
                        bool::static_type(),
                        SearchMode::static_type()])
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
                self.searchtag_files.get(),
            ];

            for tag in tag_array {
                if let Some(text) = tag.text() {
                    let prop_name = format!("by-{}", text);

                    // Connect notify signals handlers for search by properties
                    obj.connect_notify(Some(&prop_name), move |header, _| {
                        if !header.block_notify() {
                            header.emit_search_changed_signal();
                        }
                    });

                    // Bind search by properties to search tag visibility
                    obj.bind_property(&prop_name, &tag, "visible")
                        .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                        .build();
                }
            }

            // Connect notify signal handler for search mode property
            obj.connect_notify(Some("mode"), move |header, _| {
                header.emit_search_changed_signal();
            });

            // Bind search mode property to search mode tag visibility
            obj.bind_property("mode", &self.searchtag_all.get(), "visible")
                .transform_to(move |_, mode: SearchMode| Some(mode == SearchMode::All))
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            obj.bind_property("mode", &self.searchtag_any.get(), "visible")
                .transform_to(move |_, mode: SearchMode| Some(mode == SearchMode::Any))
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            obj.bind_property("mode", &self.searchtag_exact.get(), "visible")
                .transform_to(move |_, mode: SearchMode| Some(mode == SearchMode::Exact))
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();

            // Connect notify signal handler for search active property
            obj.connect_notify(Some("active"), |header, _| {
                let imp = header.imp();

                if header.active() {
                    imp.stack.set_visible_child_name("search");

                    imp.search_entry.grab_focus();
                } else {
                    imp.search_entry.set_text("");

                    imp.stack.set_visible_child_name("title");
                }

                header.emit_by_name::<()>("search-activated", &[&header.active()]);
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
            self.obj().set_active(true);
        }

        #[template_callback]
        fn on_search_changed(&self) {
            self.obj().emit_search_changed_signal();
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
// PUBLIC IMPLEMENTATION: SearchHeader
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchHeader(ObjectSubclass<imp::SearchHeader>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SearchHeader {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder()
            .build()
    }

    //-----------------------------------
    // Public signal emit helper function
    //-----------------------------------
    pub fn emit_search_changed_signal(&self) {
        self.emit_by_name::<()>("search-changed",
            &[&self.imp().search_entry.text().to_string(),
            &self.by_name(),
            &self.by_desc(),
            &self.by_group(),
            &self.by_deps(),
            &self.by_optdeps(),
            &self.by_provides(),
            &self.by_files(),
            &self.mode()]);
    }
}
