use gtk::glib;
use adw::subclass::prelude::*;
use gtk::prelude::*;

//------------------------------------------------------------------------------
// ENUM: StatusItemState
//------------------------------------------------------------------------------
#[derive(Debug)]
#[repr(u32)]
pub enum StatusItemState {
    Updates(Option<String>, u32),
    Reset,
    Checking,
}

impl Default for StatusItemState {
    fn default() -> Self {
        Self::Updates(None, 0)
    }
}

//------------------------------------------------------------------------------
// MODULE: StatusItemIndicator
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/status_item_indicator.ui")]
    pub struct StatusItemIndicator {
        #[template_child]
        pub(super) error_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub(super) spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) error_label: TemplateChild<gtk::Label>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for StatusItemIndicator {
        const NAME: &'static str = "StatusItemIndicator";
        type Type = super::StatusItemIndicator;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("StatusItemIndicator");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for StatusItemIndicator {}
    impl WidgetImpl for StatusItemIndicator {}
    impl BoxImpl for StatusItemIndicator {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: StatusItemIndicator
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct StatusItemIndicator(ObjectSubclass<imp::StatusItemIndicator>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl StatusItemIndicator {
    //---------------------------------------
    // Public set state function
    //---------------------------------------
    pub fn set_state(&self, state: StatusItemState) {
        let imp = self.imp();

        match state {
            StatusItemState::Updates(error, count) => {
                imp.spinner.set_visible(false);

                imp.count_label.set_visible(count != 0);
                imp.count_label.set_label(&count.to_string());

                if let Some(error) = error {
                    imp.error_label.set_label(&error);
                    imp.error_button.set_visible(true);
                } else {
                    imp.error_button.set_visible(false);
                }
            },
            StatusItemState::Reset => {
                imp.spinner.set_visible(false);
                imp.count_label.set_visible(false);
                imp.error_button.set_visible(false);
            }
            StatusItemState::Checking => {
                imp.error_button.set_visible(false);
                imp.count_label.set_visible(false);
                imp.spinner.set_visible(true);
            }
        }
    }
}

impl Default for StatusItemIndicator {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder()
            .build()
    }
}
