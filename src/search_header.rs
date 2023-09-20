use std::cell::{Cell, RefCell};
use core::time::Duration;

use gtk::{glib, gio, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::clone;
use glib::once_cell::sync::Lazy;

use crate::search_tag::SearchTag;

//------------------------------------------------------------------------------
// ENUM: SearchMode
//------------------------------------------------------------------------------
#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "SearchMode")]
pub enum SearchMode {
    All = 0,
    Any = 1,
    Exact = 2,
}

impl Default for SearchMode {
    fn default() -> Self {
        SearchMode::All
    }
}

//------------------------------------------------------------------------------
// FLAGS: SearchFlags
//------------------------------------------------------------------------------
#[glib::flags(name = "SearchFlags")]
pub enum SearchFlags {
    NAME     = 0b00000001,
    DESC     = 0b00000010,
    GROUP    = 0b00000100,
    DEPS     = 0b00001000,
    OPTDEPS  = 0b00010000,
    PROVIDES = 0b00100000,
    FILES    = 0b01000000,
}

impl Default for SearchFlags {
    fn default() -> Self {
        SearchFlags::NAME
    }
}

impl SearchFlags {
    pub fn from_nick(nick: &str) -> Self {
        let f_class = glib::FlagsClass::new::<Self>();

        f_class.from_nick_string(&nick)
            .map_or(Self::empty(), |value| Self::from_bits_truncate(value))
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
        pub search_text: TemplateChild<gtk::Text>,

        #[template_child]
        pub tag_mode: TemplateChild<SearchTag>,

        #[template_child]
        pub tag_flags_box: TemplateChild<gtk::Box>,

        #[template_child]
        pub clear_button: TemplateChild<gtk::Button>,

        pub capture_widget: RefCell<Option<gtk::Widget>>,
        pub capture_controller: RefCell<gtk::EventControllerKey>,

        #[property(get, set, nullable)]
        title: RefCell<Option<String>>,

        #[property(get, set)]
        active: Cell<bool>,

        #[property(get, set, builder(SearchMode::default()))]
        mode: Cell<SearchMode>,

        #[property(get, set, default = SearchFlags::default(), construct)]
        flags: Cell<SearchFlags>,

        #[property(get, set, default = 150, construct)]
        delay: Cell<u64>,

        pub delay_source_id: RefCell<Option<glib::SourceId>>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchHeader {
        const NAME: &'static str = "SearchHeader";
        type Type = super::SearchHeader;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            SearchTag::ensure_type();
            SearchMode::ensure_type();

            klass.bind_template();
            klass.set_layout_manager_type::<gtk::BoxLayout>();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SearchHeader {
        //-----------------------------------
        // Custom signals
        //-----------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder("changed")
                        .param_types([
                            String::static_type(),
                            SearchFlags::static_type(),
                            SearchMode::static_type()])
                        .build(),
                    Signal::builder("activated")
                        .param_types([bool::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }

        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_signals();
            obj.setup_actions();
            obj.setup_shortcuts();
        }

        //-----------------------------------
        // Dispose function
        //-----------------------------------
        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for SearchHeader {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SearchHeader
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchHeader(ObjectSubclass<imp::SearchHeader>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SearchHeader {
    //-----------------------------------
    // New function
    //-----------------------------------
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    //-----------------------------------
    // Setup widgets
    //-----------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind title property to title widget
        self.bind_property("title", &imp.title_widget.get(), "title")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind search text to clear button visibility
        imp.search_text.bind_property("text", &imp.clear_button.get(), "visible")
            .transform_to(|_, text: &str| Some(text != ""))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind flags property to search tag visibility
        let mut widget = imp.tag_flags_box.first_child();

        while let Some(tag) = widget.and_downcast::<SearchTag>() {
            self.bind_property("flags", &tag, "visible")
                .transform_to(move |binding, flags: SearchFlags| {
                    let tag = binding.target()
                        .and_downcast::<SearchTag>()
                        .expect("Must be a 'SearchTag'");

                    Some(flags.contains(SearchFlags::from_nick(&tag.text())))
                })
                .transform_from(move |binding, visible: bool| {
                    let header = binding.source()
                        .and_downcast::<SearchHeader>()
                        .expect("Must be a 'SearchHeader'");

                    let tag = binding.target()
                        .and_downcast::<SearchTag>()
                        .expect("Must be a 'SearchTag'");

                    let mut flags = header.flags();

                    flags.set(SearchFlags::from_nick(&tag.text()), visible);

                    Some(flags)
                })
                .flags(glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL)
                .build();
            
            widget = tag.next_sibling();
        }
    }

    //-----------------------------------
    // Setup signals
    //-----------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();
        
        // Search active property notify signal
        self.connect_active_notify(|header| {
            let imp = header.imp();

            if header.active() {
                imp.stack.set_visible_child_name("search");

                imp.search_text.grab_focus_without_selecting();
            } else {
                imp.search_text.set_text("");

                imp.stack.set_visible_child_name("title");
            }

            header.emit_by_name::<()>("activated", &[&header.active()]);
        });

        // Search mode property notify signal
        self.connect_mode_notify(|header| {
            if let Some((_, value)) = glib::EnumValue::from_value(&header.mode().to_value()) {
                header.imp().tag_mode.set_text(Some(value.nick()));

                header.emit_changed_signal();
            }
        });

        // Search flags property notify signal
        self.connect_flags_notify(|header| {
            header.emit_changed_signal();
        });

        // Search text changed signal
        imp.search_text.connect_changed(clone!(@weak self as obj, @weak imp => move |search_text| {
            // Remove delay timer if present
            if let Some(delay_id) = imp.delay_source_id.take() {
                delay_id.remove();
            }

            if search_text.text() == "" {
                obj.emit_changed_signal();
            } else {
                // Start delay timer
                let delay_id = glib::timeout_add_local_once(
                    Duration::from_millis(obj.delay()),
                    clone!(@weak imp => move || {
                        obj.emit_changed_signal();

                        imp.delay_source_id.take();
                    })
                );

                imp.delay_source_id.replace(Some(delay_id));
            }
        }));

        // Clear button clicked signal
        imp.clear_button.connect_clicked(clone!(@weak imp => move |_| {
            imp.search_text.set_text("");
        }));
    }

    //-----------------------------------
    // Emit changed signal helper function
    //-----------------------------------
    fn emit_changed_signal(&self) {
        let imp = self.imp();

        self.emit_by_name::<()>("changed",
            &[
                &imp.search_text.text(),
                &self.flags(),
                &self.mode()
            ]);
    }

    //-----------------------------------
    // Setup actions
    //-----------------------------------
    fn setup_actions(&self) {
        // Add search mode stateful action
        let mode_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("set-mode")
            .parameter_type(Some(&String::static_variant_type()))
            .state("all".to_variant())
            .change_state(clone!(@weak self as obj => move |_, action, param| {
                let param = param
                    .expect("Must be a 'Variant'")
                    .get::<String>()
                    .expect("Must be a 'String'");
    
                match param.as_str() {
                    "all" => {
                        obj.set_mode(SearchMode::All);
                        action.set_state(&param.to_variant());
                    },
                    "any" => {
                        obj.set_mode(SearchMode::Any);
                        action.set_state(&param.to_variant());
                    },
                    "exact" => {
                        obj.set_mode(SearchMode::Exact);
                        action.set_state(&param.to_variant());
                    },
                    _ => unreachable!()
                }
            }))
            .build();

        // Add cycle search mode action
        let cycle_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("cycle-mode")
            .activate(|group, _, _| {
                if let Some(mode_action) = group.lookup_action("set-mode") {
                    let state = mode_action.state()
                        .expect("Must be a 'Variant'")
                        .get::<String>()
                        .expect("Must be a 'String'");

                    match state.as_str() {
                        "all" => mode_action.change_state(&"any".to_variant()),
                        "any" => mode_action.change_state(&"exact".to_variant()),
                        "exact" => mode_action.change_state(&"all".to_variant()),
                        _ => unreachable!()
                    };
                }
            })
            .build();

        // Add select all search flags action
        let all_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("all-flags")
            .activate(clone!(@weak self as obj => move |_, _, _| {
                obj.set_flags(SearchFlags::all());
            }))
            .build();

        // Add reset search flags action
        let reset_action = gio::ActionEntry::<gio::SimpleActionGroup>::builder("reset-flags")
            .activate(clone!(@weak self as obj => move |_, _, _| {
                obj.set_flags(SearchFlags::NAME);
            }))
            .build();

        // Add actions to search action group
        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        search_group.add_action_entries([mode_action, cycle_action, all_action, reset_action]);

        // Add search flags stateful actions
        let flags_class = glib::FlagsClass::new::<SearchFlags>();

        for f in flags_class.values() {
            let flag = SearchFlags::from_bits_truncate(f.value());

            // Create stateful action
            let flag_action = gio::SimpleAction::new_stateful(&format!("flag-{}", f.nick()), None, &(flag == SearchFlags::NAME).to_variant());

            flag_action.connect_activate(clone!(@weak self as obj, @strong flag => move |_, _| {
                obj.set_flags(obj.flags() ^ flag);
            }));

            // Add action to search group
            search_group.add_action(&flag_action);

            // Bind search header flags property to action state
            self.bind_property("flags", &flag_action, "state")
                .transform_to(move |_, flags: SearchFlags| Some(flags.contains(flag).to_variant()))
                .flags(glib::BindingFlags::SYNC_CREATE)
                .build();
        }
    }

    //-----------------------------------
    // Setup shortcuts
    //-----------------------------------
    fn setup_shortcuts(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();

        // Add cycle search mode shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>M"),
            Some(gtk::NamedAction::new("search.cycle-mode"))
        ));

        // Add select all search flags action/shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>L"),
            Some(gtk::NamedAction::new("search.all-flags"))
        ));

        // Add reset search flags action/shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<ctrl>R"),
            Some(gtk::NamedAction::new("search.reset-flags"))
        ));

        // Add search flags shortcuts
        let flags_class = glib::FlagsClass::new::<SearchFlags>();

        for (i, f) in flags_class.values().iter().enumerate() {
            controller.add_shortcut(gtk::Shortcut::new(
                gtk::ShortcutTrigger::parse_string(&format!("<ctrl>{}", i+1)),
                Some(gtk::NamedAction::new(&format!("search.flag-{}", f.nick()))))
            );
        }

        // Add shortcut controller to search header
        self.add_controller(controller);
    }

    //-----------------------------------
    // Public set capture widget function
    //-----------------------------------
    pub fn set_key_capture_widget(&self, widget: gtk::Widget) {
        let imp = self.imp();

        if let Some(current_widget) = &*imp.capture_widget.borrow() {
            current_widget.remove_controller(&*imp.capture_controller.borrow());
        }

        let controller = gtk::EventControllerKey::new();

        controller.connect_key_pressed(clone!(@weak self as header => @default-return glib::Propagation::Proceed, move |controller, _, _, state| {
            if !(state.contains(gdk::ModifierType::ALT_MASK) || state.contains(gdk::ModifierType::CONTROL_MASK))
            {
                if controller.forward(&header.imp().search_text.get()) {
                    header.set_active(true);
                }
            }

            glib::Propagation::Proceed
        }));

        widget.add_controller(controller.clone());

        imp.capture_widget.replace(Some(widget));

        imp.capture_controller.replace(controller);
    }
}
