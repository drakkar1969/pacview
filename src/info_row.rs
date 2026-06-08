use std::cell::Cell;
use std::marker::PhantomData;

use gtk::{glib, gdk};
use gtk::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, GString, RustClosure};
use gdk::{Key, ModifierType};

use strum::AsRefStr;

use crate::text_widget::{TextWidget, LINK_SPACER};

//------------------------------------------------------------------------------
// ENUM: PropID
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, Hash, glib::Enum, AsRefStr)]
#[repr(u32)]
#[enum_type(name = "PropID")]
pub enum PropID {
    #[default]
    #[strum(serialize = "Popularity")]
    Popularity,
    #[strum(serialize = "Out of Date")]
    OutOfDate,
    #[strum(serialize = "Package URL")]
    PackageUrl,
    #[strum(serialize = "URL")]
    Url,
    #[strum(serialize = "Groups")]
    Groups,
    #[strum(serialize = "Dependencies")]
    Dependencies,
    #[strum(serialize = "Optional")]
    Optional,
    #[strum(serialize = "Make")]
    Make,
    #[strum(serialize = "Required By")]
    RequiredBy,
    #[strum(serialize = "Optional For")]
    OptionalFor,
    #[strum(serialize = "Provides")]
    Provides,
    #[strum(serialize = "Conflicts With")]
    ConflictsWith,
    #[strum(serialize = "Replaces")]
    Replaces,
    #[strum(serialize = "Licenses")]
    Licenses,
    #[strum(serialize = "Architecture")]
    Architecture,
    #[strum(serialize = "Packager")]
    Packager,
    #[strum(serialize = "Build Date")]
    BuildDate,
    #[strum(serialize = "Install Date")]
    InstallDate,
    #[strum(serialize = "Download Size")]
    DownloadSize,
    #[strum(serialize = "Install Script")]
    InstallScript,
    #[strum(serialize = "Validation")]
    Validation,
}

//------------------------------------------------------------------------------
// ENUM: PropType
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PropType")]
pub enum PropType {
    #[default]
    Text,
    Link,
    Packager,
    LinkList,
    Error,
}

//------------------------------------------------------------------------------
// ENUM: ValueType
//------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy)]
pub enum ValueType<'a> {
    Str(&'a str),
    StrOpt(&'a str),
    StrOptNum(&'a str, i64),
    Vec(&'a [String]),
    VecOpt(&'a [String]),
    VecOptJoin(&'a [String])
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
        pub(super) prop_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) prop_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) expand_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub(super) value_widget: TemplateChild<TextWidget>,

        #[property(get = Self::label)]
        label: PhantomData<GString>,
        #[property(get = Self::value)]
        value: PhantomData<GString>,
        #[property(get, set)]
        has_selection: Cell<bool>,
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
    impl ObjectImpl for InfoRow {
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

    impl WidgetImpl for InfoRow {}
    impl ListBoxRowImpl for InfoRow {}

    impl InfoRow {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Selection actions
            klass.install_action("text.select-all", None, |row, _, _| {
                row.imp().value_widget.select_all();
            });

            klass.install_action("text.select-none", None, |row, _, _| {
                row.imp().value_widget.select_none();
            });

            // Copy action
            klass.install_action("text.copy", None, |row, _, _| {
                if let Some(text) = row.imp().value_widget.selected_text() {
                    row.clipboard().set_text(&text);
                }
            });

            // Expand/contract actions
            klass.install_action("text.expand", None, |row, _, _| {
                let widget = &row.imp().value_widget;

                if widget.can_expand() && !widget.expanded() {
                    widget.set_expanded(true);
                }
            });

            klass.install_action("text.contract", None, |row, _, _| {
                let widget = &row.imp().value_widget;

                if widget.can_expand() && widget.expanded() {
                    widget.set_expanded(false);
                }
            });

            // Link actions
            klass.install_action("text.previous-link", None, |row, _, _| {
                row.imp().value_widget.select_previous_link();
            });

            klass.install_action("text.next-link", None, |row, _, _| {
                row.imp().value_widget.select_next_link();
            });

            klass.install_action("text.activate-link", None, |row, _, _| {
                let widget = &row.imp().value_widget;

                if let Some(link) = widget.active_link() {
                    widget.handle_link(&link);
                }
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Select all/none key bindings
            klass.add_binding_action(Key::A, ModifierType::CONTROL_MASK, "text.select-all");

            klass.add_binding_action(Key::A, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "text.select-none");

            // Copy key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK, "text.copy");

            // Expand/contract key bindings
            klass.add_binding_action(Key::plus, ModifierType::CONTROL_MASK, "text.expand");
            klass.add_binding_action(Key::KP_Add, ModifierType::CONTROL_MASK, "text.expand");

            klass.add_binding_action(Key::minus, ModifierType::CONTROL_MASK, "text.contract");
            klass.add_binding_action(Key::KP_Subtract, ModifierType::CONTROL_MASK, "text.contract");

            // Previous/next link key bindings
            klass.add_binding_action(Key::Left, ModifierType::NO_MODIFIER_MASK, "text.previous-link");
            klass.add_binding_action(Key::KP_Left, ModifierType::NO_MODIFIER_MASK, "text.previous-link");

            klass.add_binding_action(Key::Right, ModifierType::NO_MODIFIER_MASK, "text.next-link");
            klass.add_binding_action(Key::KP_Right, ModifierType::NO_MODIFIER_MASK, "text.next-link");

            // Activate link key bindings
            klass.add_binding_action(Key::Return, ModifierType::NO_MODIFIER_MASK, "text.activate-link");
            klass.add_binding_action(Key::KP_Enter, ModifierType::NO_MODIFIER_MASK, "text.activate-link");
        }

        //---------------------------------------
        // Property getters
        //---------------------------------------
        fn label(&self) -> GString {
            self.prop_label.label()
        }

        fn value(&self) -> GString {
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
    pub fn new(id: PropID, ptype: PropType, link_handler: Option<&RustClosure>) -> Self {
        let obj: Self = glib::Object::builder().build();

        let imp = obj.imp();

        imp.prop_label.set_label(id.as_ref());
        imp.value_widget.set_ptype(ptype);

        if let Some(handler) = link_handler {
            imp.value_widget.connect_closure("package-link", false, handler.clone());
        }

        obj
    }

    //---------------------------------------
    // Public set value function
    //---------------------------------------
    pub fn set_value(&self, value: ValueType) {
        let imp = self.imp();

        let visible = match value {
            ValueType::Str(_) | ValueType::Vec(_) => true,
            ValueType::StrOpt(s) => !s.is_empty(),
            ValueType::StrOptNum(_, i) => i != 0,
            ValueType::VecOpt(v) | ValueType::VecOptJoin(v) => !v.is_empty(),
        };

        self.set_visible(visible);

        if visible {
            match value {
                ValueType::Str(s) | ValueType::StrOpt(s) | ValueType::StrOptNum(s, _) => {
                    imp.value_widget.set_text(s);
                }
                ValueType::Vec(v) | ValueType::VecOpt(v) => {
                    imp.value_widget.set_text(v.join(LINK_SPACER));
                }
                ValueType::VecOptJoin(v) => {
                    imp.value_widget.set_text(v.join(" | "));
                }
            }
        }
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Value widget expanded property notify
        imp.value_widget.connect_expanded_notify(clone!(
            #[weak] imp,
            move |widget| {
                if widget.expanded() {
                    imp.prop_box.add_css_class("active");
                } else {
                    imp.prop_box.remove_css_class("active");
                }
            }
        ));

        // Has selection property notify signal
        self.connect_has_selection_notify(|row| {
            row.action_set_enabled("text.copy", row.has_selection());
        });
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind has focus property to value widget focused property
        self.bind_property("has-focus", &imp.value_widget.get(), "focused")
            .sync_create()
            .build();

        // Bind value widget has selection property to has selection property
        imp.value_widget.bind_property("has-selection", self, "has-selection")
            .sync_create()
            .build();

        // Bind value widget can expand property to expand image visibility
        imp.value_widget.bind_property("can-expand", &imp.expand_image.get(), "visible")
            .sync_create()
            .build();
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        let imp = self.imp();

        // Label expand area click gesture
        let click_gesture = gtk::GestureClick::builder()
            .button(gdk::BUTTON_PRIMARY)
            .build();

        click_gesture.connect_released(clone!(
            #[weak] imp,
            move |_, n, _, _| {
                if n == 1 {
                    imp.value_widget.set_expanded(!imp.value_widget.expanded());
                }
            }
        ));

        imp.prop_box.add_controller(click_gesture);

        // Mouse drag gesture (needed to ensure row gets focus on drag)
        let drag_gesture = gtk::GestureDrag::new();

        drag_gesture.connect_drag_begin(clone!(
            #[weak(rename_to = row)] self,
            move |_, _, _| {
                row.grab_focus();
            }
        ));

        self.add_controller(drag_gesture);
    }
}
