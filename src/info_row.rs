use std::cell::Cell;
use std::sync::OnceLock;
use std::marker::PhantomData;

use gtk::{glib, gdk, graphene};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::RustClosure;
use glib::subclass::Signal;
use gdk::{Key, ModifierType};

use crate::text_widget::{TextWidget, LINK_SPACER};
use crate::enum_traits::EnumExt;

//------------------------------------------------------------------------------
// ENUM: PropID
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, Hash, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropID")]
pub enum PropID {
    #[default]
    #[enum_value(name = "Name")]
    Name,
    #[enum_value(name = "Version")]
    Version,
    #[enum_value(name = "Description")]
    Description,
    #[enum_value(name = "Popularity")]
    Popularity,
    #[enum_value(name = "Out of Date")]
    OutOfDate,
    #[enum_value(name = "Package URL")]
    PackageUrl,
    #[enum_value(name = "URL")]
    Url,
    #[enum_value(name = "Status")]
    Status,
    #[enum_value(name = "Repository")]
    Repository,
    #[enum_value(name = "Groups")]
    Groups,
    #[enum_value(name = "Dependencies")]
    Dependencies,
    #[enum_value(name = "Optional")]
    Optional,
    #[enum_value(name = "Make")]
    Make,
    #[enum_value(name = "Required By")]
    RequiredBy,
    #[enum_value(name = "Optional For")]
    OptionalFor,
    #[enum_value(name = "Provides")]
    Provides,
    #[enum_value(name = "Conflicts With")]
    ConflictsWith,
    #[enum_value(name = "Replaces")]
    Replaces,
    #[enum_value(name = "Licenses")]
    Licenses,
    #[enum_value(name = "Architecture")]
    Architecture,
    #[enum_value(name = "Packager")]
    Packager,
    #[enum_value(name = "Build Date")]
    BuildDate,
    #[enum_value(name = "Install Date")]
    InstallDate,
    #[enum_value(name = "Download Size")]
    DownloadSize,
    #[enum_value(name = "Installed Size")]
    InstalledSize,
    #[enum_value(name = "Install Script")]
    InstallScript,
    #[enum_value(name = "Validation")]
    Validation,
}

impl EnumExt for PropID {}

//------------------------------------------------------------------------------
// ENUM: PropType
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropType")]
pub enum PropType {
    #[default]
    Text,
    Title,
    Link,
    Packager,
    LinkList,
    Error,
}

//------------------------------------------------------------------------------
// ENUM: ValueType
//------------------------------------------------------------------------------
#[derive(Debug)]
pub enum ValueType<'a> {
    Str(&'a str),
    StrIcon(&'a str, Option<&'a str>),
    StrOpt(&'a str),
    StrOptNum(&'a str, i64),
    Vec(&'a [String]),
    VecOpt(&'a [String])
}

//------------------------------------------------------------------------------
// MODULE: InfoRow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoRow)]
    #[template(resource = "/com/github/PacView/ui/info_row.ui")]
    pub struct InfoRow {
        #[template_child]
        pub(super) prop_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) expand_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) value_widget: TemplateChild<TextWidget>,

        #[property(get = Self::label)]
        label: PhantomData<glib::GString>,
        #[property(get = Self::value)]
        value: PhantomData<glib::GString>,

        pub(super) id: Cell<PropID>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoRow {
        const NAME: &'static str = "InfoRow";
        type Type = super::InfoRow;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InfoRow {
        //---------------------------------------
        // Signals
        //---------------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("selection-widget")
                        .param_types([TextWidget::static_type()])
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
            obj.setup_controllers();
        }
    }

    impl WidgetImpl for InfoRow {}
    impl ListBoxRowImpl for InfoRow {}

    impl InfoRow {
        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Select all/none key bindings
            klass.add_binding(Key::A, ModifierType::CONTROL_MASK, |row| {
                row.imp().value_widget.activate_action("text.select-all", None).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::A, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, |row| {
                row.imp().value_widget.activate_action("text.select-none", None).unwrap();

                glib::Propagation::Stop
            });

            // Copy key binding
            klass.add_binding(Key::C, ModifierType::CONTROL_MASK, |row| {
                row.imp().value_widget.activate_action("text.copy", None).unwrap();

                glib::Propagation::Stop
            });

            // Expand/contract key bindings
            klass.add_binding(Key::plus, ModifierType::CONTROL_MASK, |row| {
                row.imp().value_widget.activate_action("text.expand", None).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::KP_Add, ModifierType::CONTROL_MASK, |row| {
                row.imp().value_widget.activate_action("text.expand", None).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::minus, ModifierType::CONTROL_MASK, |row| {
                row.imp().value_widget.activate_action("text.contract", None).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::KP_Subtract, ModifierType::CONTROL_MASK, |row| {
                row.imp().value_widget.activate_action("text.contract", None).unwrap();

                glib::Propagation::Stop
            });

            // Previous/next link key bindings
            klass.add_binding(Key::Left, ModifierType::NO_MODIFIER_MASK, |row| {
                row.imp().value_widget.activate_action("text.previous-link", None).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::KP_Left, ModifierType::NO_MODIFIER_MASK, |row| {
                row.imp().value_widget.activate_action("text.previous-link", None).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::Right, ModifierType::NO_MODIFIER_MASK, |row| {
                row.imp().value_widget.activate_action("text.next-link", None).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::KP_Right, ModifierType::NO_MODIFIER_MASK, |row| {
                row.imp().value_widget.activate_action("text.next-link", None).unwrap();

                glib::Propagation::Stop
            });

            // Activate link key bindings
            klass.add_binding(Key::Return, ModifierType::NO_MODIFIER_MASK, |row| {
                row.imp().value_widget.activate_action("text.activate-link", None).unwrap();

                glib::Propagation::Stop
            });

            klass.add_binding(Key::KP_Enter, ModifierType::NO_MODIFIER_MASK, |row| {
                row.imp().value_widget.activate_action("text.activate-link", None).unwrap();

                glib::Propagation::Stop
            });
        }

        //---------------------------------------
        // Property getters
        //---------------------------------------
        fn label(&self) -> glib::GString {
            self.prop_label.label()
        }

        fn value(&self) -> glib::GString {
            self.value_widget.text()
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoRow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoRow(ObjectSubclass<imp::InfoRow>)
    @extends gtk::ListBoxRow, gtk::Widget,
    @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl InfoRow {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(id: PropID, ptype: PropType) -> Self {
        let obj: Self = glib::Object::builder().build();

        let imp = obj.imp();

        imp.id.set(id);

        imp.prop_label.set_label(&id.name());
        imp.value_widget.set_ptype(ptype);

        obj
    }

    //---------------------------------------
    // Public set package link handler function
    //---------------------------------------
    pub fn set_pkg_link_handler(&self, handler: RustClosure) {
        self.imp().value_widget.connect_closure("package-link", false, handler);
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Row has focus property notify
        self.connect_has_focus_notify(|row| {
            row.imp().value_widget.set_focused(row.has_focus());
        });

        // Expand button clicked signal
        imp.expand_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                imp.value_widget.set_expanded(!imp.value_widget.expanded());
            }
        ));

        // Value widget can expand property notify
        imp.value_widget.connect_can_expand_notify(clone!(
            #[weak] imp,
            move |widget| {
                imp.expand_button.set_visible(widget.can_expand());
            }
        ));

        // Value widget expanded property notify
        imp.value_widget.connect_expanded_notify(clone!(
            #[weak] imp,
            move |widget| {
                if widget.expanded() {
                    imp.expand_button.add_css_class("active");
                } else {
                    imp.expand_button.remove_css_class("active");
                }
            }
        ));

        // Value widget has selection property notify
        imp.value_widget.connect_has_selection_notify(clone!(
            #[weak(rename_to = row)] self,
            move |widget| {
                row.emit_by_name::<()>("selection-widget", &[&widget]);
            }
        ));
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        // Mouse drag controller
        let drag_controller = gtk::GestureDrag::new();

        drag_controller.connect_drag_begin(clone!(
            #[weak(rename_to = row)] self,
            move |_, _, _| {
                row.grab_focus();
            }
        ));

        self.add_controller(drag_controller);

        // Popup menu controller
        let popup_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();

        popup_gesture.connect_pressed(clone!(
            #[weak(rename_to = row)] self,
            move |_, _, x, y| {
                let value_widget = &row.imp().value_widget;

                if let Some(point) = row.compute_point(
                    &value_widget.get(),
                    &graphene::Point::new(x as f32, y as f32)
                ) {
                    value_widget.popup_menu(f64::from(point.x()), f64::from(point.y()));
                }
            }
        ));

        self.add_controller(popup_gesture);
    }

    //---------------------------------------
    // Set icon css class helper function
    //---------------------------------------
    fn set_icon_css_class(&self, class: &str, add: bool) {
        let imp = self.imp();

        if add {
            imp.image.add_css_class(class);
        } else {
            imp.image.remove_css_class(class);
        }
    }

    //---------------------------------------
    // Public set value function
    //---------------------------------------
    pub fn set_value(&self, value: ValueType) {
        let imp = self.imp();

        let visible = match value {
            ValueType::Str(_) | ValueType::StrIcon(_, _) | ValueType::Vec(_) => true,
            ValueType::StrOpt(s) => !s.is_empty(),
            ValueType::StrOptNum(_, i) => i != 0,
            ValueType::VecOpt(v) => !v.is_empty(),
        };

        self.set_visible(visible);

        if visible {
            match value {
                ValueType::Str(s) | ValueType::StrOpt(s) | ValueType::StrOptNum(s, _) => {
                    imp.image.set_visible(false);
                    imp.value_widget.set_text(s);
                },
                ValueType::StrIcon(s, icon) => {
                    imp.image.set_visible(icon.is_some());
                    imp.image.set_icon_name(icon);
                    imp.value_widget.set_text(s);
                }
                ValueType::Vec(v) | ValueType::VecOpt(v) => {
                    imp.image.set_visible(false);
                    imp.value_widget.set_text(v.join(LINK_SPACER));
                }
            }

            let id = imp.id.get();

            if id == PropID::Version {
                self.set_icon_css_class("success", true);
            } else if id == PropID::Status {
                self.set_icon_css_class("error", imp.image.icon_name().unwrap_or_default() == "pkg-orphan");
            }
        }
    }
}
