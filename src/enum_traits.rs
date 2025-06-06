use gtk::glib;
use glib::types::StaticType;
use glib::value::ToValue;
use glib::variant::ToVariant;
use glib::{EnumValue, HasParamSpec, ParamSpecEnum, Variant};

//------------------------------------------------------------------------------
// TRAIT: EnumExt
//------------------------------------------------------------------------------
pub trait EnumExt
where Self: ToValue + StaticType + HasParamSpec<ParamSpec = ParamSpecEnum> + Sized {
    fn name(&self) -> String {
        EnumValue::from_value(&self.to_value())
            .map(|(_, enum_value)| enum_value.name().to_owned())
            .expect("Failed to get 'EnumValue'")
    }

    fn nick(&self) -> String {
        EnumValue::from_value(&self.to_value())
            .map(|(_, enum_value)| enum_value.nick().to_owned())
            .expect("Failed to get 'EnumValue'")
    }

    fn value(&self) -> u32 {
        EnumValue::from_value(&self.to_value())
            .map(|(_, enum_value)| enum_value.value() as u32)
            .expect("Failed to get 'EnumValue'")
    }

    fn nick_variant(&self) -> Variant {
        EnumValue::from_value(&self.to_value())
            .map(|(_, enum_value)| enum_value.nick().to_variant())
            .expect("Failed to get 'EnumValue'")
    }
}
