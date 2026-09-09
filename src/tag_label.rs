use std::cell::RefCell;
use std::marker::PhantomData;

use gtk::{glib, pango};
use adw::subclass::prelude::*;
use gtk::prelude::*;

use strum::AsRefStr;

//------------------------------------------------------------------------------
// ENUM: TagType
//------------------------------------------------------------------------------
#[derive(Default, Debug, Clone, Copy, glib::Enum, AsRefStr)]
#[strum(serialize_all = "lowercase")]
#[repr(u32)]
#[enum_type(name = "TagType")]
pub enum TagType {
    #[default]
    None,
    Normal,
    Accent,
    Success,
    Warning,
    Error,
}

//------------------------------------------------------------------------------
// ENUM: TagEllipsizeMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "TagEllipsizeMode")]
pub enum TagEllipsizeMode {
    #[default]
    None,
    Start,
    Middle,
    End
}

//------------------------------------------------------------------------------
// MODULE: TagLabel
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::TagLabel)]
    #[template(resource = "/com/github/PacView/ui/tag_label.ui")]
    pub struct TagLabel {
        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,

        #[property(set = Self::set_tag_type, builder(TagType::default()))]
        tag_type: PhantomData<TagType>,
        #[property(get, set)]
        text: RefCell<String>,
        #[property(set = Self::set_ellipsize, builder(TagEllipsizeMode::default()))]
        ellipsize: PhantomData<TagEllipsizeMode>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for TagLabel {
        const NAME: &'static str = "TagLabel";
        type Type = super::TagLabel;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("taglabel");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for TagLabel {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
        }
    }

    impl WidgetImpl for TagLabel {}
    impl BinImpl for TagLabel {}

    impl TagLabel {
        //---------------------------------------
        // Property setters
        //---------------------------------------
        fn set_tag_type(&self, tag_type: TagType) {
            let obj = self.obj();

            match tag_type {
                TagType::None => { obj.set_css_classes(&[]); },
                TagType::Normal => { obj.set_css_classes(&["tag"]); },
                _ => { obj.set_css_classes(&["tag", tag_type.as_ref()]); }
            }
        }

        fn set_ellipsize(&self, mode: TagEllipsizeMode) {
            let pango_mode = match mode {
                TagEllipsizeMode::None => pango::EllipsizeMode::None,
                TagEllipsizeMode::Start => pango::EllipsizeMode::Start,
                TagEllipsizeMode::Middle => pango::EllipsizeMode::Middle,
                TagEllipsizeMode::End => pango::EllipsizeMode::End,
            };

            self.label.set_ellipsize(pango_mode);
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: TagLabel
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct TagLabel(ObjectSubclass<imp::TagLabel>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl TagLabel {
    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind text property to label
        self.bind_property("text", &imp.label.get(), "label")
            .sync_create()
            .build();
    }
}
