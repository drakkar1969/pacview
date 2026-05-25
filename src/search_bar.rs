use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::sync::OnceLock;
use core::time::Duration;

use gtk::{glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::subclass::Signal;
use glib::{clone, closure_local, GString};
use gdk::{Key, ModifierType};

use strum::{FromRepr, EnumIter, IntoEnumIterator, AsRefStr};

use crate::search_tag::SearchTag;

//------------------------------------------------------------------------------
// ENUM: SearchProp
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, FromRepr, EnumIter, AsRefStr)]
#[strum(serialize_all = "kebab-case")]
#[repr(u32)]
#[enum_type(name = "SearchProp")]
pub enum SearchProp {
    #[default]
    #[enum_value(name = "Name")]
    Name,
    #[enum_value(name = "Name or Description")]
    NameDesc,
    #[enum_value(name = "Groups")]
    Groups,
    #[enum_value(name = "Dependencies")]
    Deps,
    #[enum_value(name = "Optional Dependencies")]
    Optdeps,
    #[enum_value(name = "Provides")]
    Provides,
    #[enum_value(name = "Files")]
    Files,
}

//------------------------------------------------------------------------------
// MODULE: SearchBar
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SearchBar)]
    #[template(resource = "/com/github/PacView/ui/search_bar.ui")]
    pub struct SearchBar {
        #[template_child]
        pub(super) revealer: TemplateChild<gtk::Revealer>,

        #[template_child]
        pub(super) search_image: TemplateChild<gtk::Image>,

        #[template_child]
        pub(super) prop_tag: TemplateChild<SearchTag>,

        #[template_child]
        pub(super) search_text: TemplateChild<gtk::Text>,

        #[template_child]
        pub(super) clear_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) exact_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) error_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) error_label: TemplateChild<gtk::Label>,

        pub(super) has_capture_widget: Cell<bool>,

        #[property(get, set)]
        enabled: Cell<bool>,

        #[property(get = Self::text)]
        text: PhantomData<GString>,
        #[property(get, set, builder(SearchProp::default()))]
        prop: Cell<SearchProp>,
        #[property(get, set)]
        exact: Cell<bool>,
        #[property(get, set, builder(SearchProp::default()))]
        default_prop: Cell<SearchProp>,
        #[property(get, set)]
        default_exact: Cell<bool>,

        #[property(get, set, default = 150, construct)]
        delay: Cell<u64>,

        #[property(get, set)]
        searching: Cell<bool>,

        pub(super) spinner: RefCell<Option<adw::SpinnerPaintable>>,

        pub(super) search_delay_id: RefCell<Option<glib::SourceId>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchBar {
        const NAME: &'static str = "SearchBar";
        type Type = super::SearchBar;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("pkgsearchbar");

            // Install actions
            Self::install_actions(klass);

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SearchBar {
        //---------------------------------------
        // Signals
        //---------------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("changed")
                        .build(),
                    Signal::builder("aur-search")
                        .build(),
                ]
            })
        }

        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_widgets();
            obj.setup_controllers();
        }
    }

    impl WidgetImpl for SearchBar {}
    impl BinImpl for SearchBar {}

    impl SearchBar {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Search prop property action
            klass.install_property_action("search.set-prop", "prop");

            // Search exact property action
            klass.install_property_action("search.set-exact", "exact");

            // Cycle search property action
            klass.install_action("search.cycle-prop", None, |bar, _, _| {
                let new_prop = SearchProp::iter().cycle()
                    .skip_while(|&prop| prop != bar.prop())
                    .nth(1)
                    .expect("Failed to get 'SearchProp'");

                bar.set_prop(new_prop);
            });

            // Reverse cycle search property action
            klass.install_action("search.reverse-cycle-prop", None, |bar, _, _| {
                let new_prop = SearchProp::iter().rev().cycle()
                    .skip_while(|&prop| prop != bar.prop())
                    .nth(1)
                    .expect("Failed to get 'SearchProp'");

                bar.set_prop(new_prop);
            });

            // Reset search params action
            klass.install_action("search.reset-params", None, |bar, _, _| {
                bar.set_prop(bar.default_prop());
                bar.set_exact(bar.default_exact());
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Cycle search prop key bindings
            klass.add_binding_action(Key::P, ModifierType::CONTROL_MASK, "search.cycle-prop");
            klass.add_binding_action(Key::P, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "search.reverse-cycle-prop");

            // Search prop numbered key bindings
            for (i, prop) in SearchProp::iter().enumerate() {
                let trigger = gtk::ShortcutTrigger::parse_string(&format!("<ctrl>{}", i+1));
                let action = gtk::NamedAction::new("search.set-prop");
                let args = Some(&prop.as_ref().to_variant());

                let shortcut = gtk::Shortcut::new(trigger, Some(action));
                shortcut.set_arguments(args);

                klass.add_shortcut(&shortcut);
            }

            // Search exact key binding
            klass.add_binding_action(Key::W, ModifierType::CONTROL_MASK, "search.set-exact");

            // Reset search params key binding
            klass.add_binding_action(Key::R, ModifierType::CONTROL_MASK, "search.reset-params");
        }

        //---------------------------------------
        // Property getter
        //---------------------------------------
        fn text(&self) -> GString {
            self.search_text.text()
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SearchBar
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchBar(ObjectSubclass<imp::SearchBar>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SearchBar {
    //---------------------------------------
    // Public set AUR status function
    //---------------------------------------
    pub fn set_aur_status(&self, status: Result<(), String>) {
        let imp = self.imp();

        if let Err(mut error) = status {
            self.add_css_class("error");

            error.pop();

            imp.error_label.set_label(&error);
            imp.error_button.set_visible(true);
        } else {
            self.remove_css_class("error");

            imp.error_label.set_label("");
            imp.error_button.set_visible(false);
        }
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search enabled property notify signal
        self.connect_enabled_notify(|bar| {
            let imp = bar.imp();

            let enabled = bar.enabled();

            imp.revealer.set_reveal_child(enabled);

            if enabled {
                imp.search_text.grab_focus_without_selecting();
            } else {
                imp.search_text.set_text("");
            }
        });

        // Searching property notify signal
        self.connect_searching_notify(|bar| {
            let imp = bar.imp();

            if bar.searching() {
                imp.search_image.set_paintable(imp.spinner.borrow().as_ref());
            } else {
                imp.search_image.set_icon_name(Some("edit-find-symbolic"));
            }
        });

        // Search prop property notify signal
        self.connect_prop_notify(|bar| {
            let imp = bar.imp();

            imp.prop_tag.set_text(bar.prop().as_ref());

            if !imp.search_text.text().is_empty() {
                bar.emit_by_name::<()>("changed", &[]);
            }
        });

        // Search exact property notify signal
        self.connect_exact_notify(|bar| {
            let imp = bar.imp();

            if !imp.search_text.text().is_empty() {
                bar.emit_by_name::<()>("changed", &[]);
            }
        });

        // Prop tag clicked signal
        imp.prop_tag.connect_closure("clicked", false, closure_local!(
            #[weak(rename_to = bar)] self,
            move |_: SearchTag, shift: bool| {
                if shift {
                    bar.activate_action("search.reverse-cycle-prop", None).unwrap();
                } else {
                    bar.activate_action("search.cycle-prop", None).unwrap();
                }
            }
        ));

        // Search text changed signal
        imp.search_text.connect_changed(clone!(
            #[weak(rename_to = bar)] self,
            #[weak] imp,
            move |search_text| {
                let text = search_text.text();

                imp.clear_image.set_visible(!text.is_empty());

                // Remove delay timer if present
                if let Some(delay_id) = imp.search_delay_id.take() {
                    delay_id.remove();
                }

                if text.is_empty() {
                    bar.set_aur_status(Ok(()));

                    bar.emit_by_name::<()>("changed", &[]);
                    bar.emit_by_name::<()>("aur-search", &[]);
                } else {
                    // Start delay timer
                    let delay_id = glib::timeout_add_local_once(
                        Duration::from_millis(bar.delay()),
                        clone!(
                            #[weak] imp,
                            move || {
                                bar.emit_by_name::<()>("changed", &[]);

                                imp.search_delay_id.take();
                            }
                        )
                    );

                    imp.search_delay_id.replace(Some(delay_id));
                }
            }
        ));

        // Search text activate signal
        imp.search_text.connect_activate(clone!(
            #[weak(rename_to = bar)] self,
            move |search_text| {
                if !search_text.text().is_empty() {
                    bar.emit_by_name::<()>("aur-search", &[]);
                }
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        imp.spinner.replace(Some(adw::SpinnerPaintable::new(Some(&imp.search_image.get()))));

        // Bind exact property to button state
        self.bind_property("exact", &imp.exact_button.get(), "active")
            .bidirectional()
            .sync_create()
            .build();
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        let gesture_click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();

        gesture_click.connect_released(clone!(
            #[weak] imp,
            move |_, n_press, _, _| {
                if n_press == 1 {
                    imp.search_text.set_text("");

                    if !imp.search_text.has_focus() {
                        imp.search_text.grab_focus_without_selecting();
                    }
                }
            }
        ));

        imp.clear_image.add_controller(gesture_click);
    }

    //---------------------------------------
    // Public set capture widget function
    //---------------------------------------
    pub fn set_key_capture_widget(&self, widget: &gtk::Widget) {
        let imp = self.imp();

        if !imp.has_capture_widget.get() {
            let controller = gtk::EventControllerKey::new();

            controller.connect_key_pressed(clone!(
                #[weak(rename_to = bar)] self,
                #[upgrade_or] glib::Propagation::Proceed,
                move |controller, _, _, state| {
                    if !(bar.enabled() || state.contains(ModifierType::ALT_MASK)
                        || state.contains(ModifierType::CONTROL_MASK))
                        && controller.forward(&bar.imp().search_text.get()) {
                            bar.set_enabled(true);
                        }

                    glib::Propagation::Proceed
                }
            ));

            widget.add_controller(controller);

            imp.has_capture_widget.set(true);
        }
    }
}

impl Default for SearchBar {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
