use gtk::glib;
use adw::subclass::prelude::*;
use gtk::prelude::*;

//------------------------------------------------------------------------------
// ENUM: SidebarItemState
//------------------------------------------------------------------------------
#[derive(Debug)]
#[repr(u32)]
pub enum SidebarItemState {
    Updates(Option<String>, u32),
    Reset,
    Checking,
}

impl Default for SidebarItemState {
    fn default() -> Self {
        Self::Updates(None, 0)
    }
}

//------------------------------------------------------------------------------
// MODULE: SidebarItemIndicator
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/sidebar_item_indicator.ui")]
    pub struct SidebarItemIndicator {
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
    impl ObjectSubclass for SidebarItemIndicator {
        const NAME: &'static str = "SidebarItemIndicator";
        type Type = super::SidebarItemIndicator;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("SidebarItemIndicator");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SidebarItemIndicator {}
    impl WidgetImpl for SidebarItemIndicator {}
    impl BoxImpl for SidebarItemIndicator {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SidebarItemIndicator
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SidebarItemIndicator(ObjectSubclass<imp::SidebarItemIndicator>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl SidebarItemIndicator {
    //---------------------------------------
    // Public set state function
    //---------------------------------------
    pub fn set_state(&self, state: SidebarItemState) {
        let imp = self.imp();

        match state {
            SidebarItemState::Updates(error, count) => {
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
            SidebarItemState::Reset => {
                imp.spinner.set_visible(false);
                imp.count_label.set_visible(false);
                imp.error_button.set_visible(false);
            }
            SidebarItemState::Checking => {
                imp.error_button.set_visible(false);
                imp.count_label.set_visible(false);
                imp.spinner.set_visible(true);
            }
        }
    }
}

impl Default for SidebarItemIndicator {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder()
            .build()
    }
}
