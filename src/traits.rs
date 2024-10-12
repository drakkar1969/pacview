use glib::types::StaticType;
use glib::value::ToValue;
use glib::variant::ToVariant;
use glib::{EnumClass, EnumValue, HasParamSpec, ParamSpecEnum, Variant};

use num::ToPrimitive;

//------------------------------------------------------------------------------
// TRAIT: EnumValueExt
//------------------------------------------------------------------------------
pub trait EnumValueExt
where Self: ToValue + StaticType + HasParamSpec<ParamSpec = ParamSpecEnum> + Sized {
    fn name(&self) -> String {
        EnumValue::from_value(&self.to_value())
            .map(|(_, enum_value)| enum_value.name().to_string())
            .expect("Could not get 'EnumValue'")
    }

    fn nick(&self) -> String {
        EnumValue::from_value(&self.to_value())
            .map(|(_, enum_value)| enum_value.nick().to_string())
            .expect("Could not get 'EnumValue'")
    }

    fn value(&self) -> u32 {
        EnumValue::from_value(&self.to_value())
            .and_then(|(_, enum_value)| enum_value.value().to_u32())
            .expect("Could not get 'EnumValue'")
    }

    fn variant_nick(&self) -> Variant {
        EnumValue::from_value(&self.to_value())
            .map(|(_, enum_value)| enum_value.nick().to_variant())
            .expect("Could not get 'EnumValue'")
    }
}

//------------------------------------------------------------------------------
// TRAIT: EnumClassExt
//------------------------------------------------------------------------------
pub trait EnumClassExt
where Self: StaticType + HasParamSpec<ParamSpec = ParamSpecEnum> + Sized {
    fn enum_class() -> EnumClass {
        EnumClass::new::<Self>()
    }

    fn previous_nick(nick: &str) -> String {
        let enum_class = Self::enum_class();

        let mut iter = enum_class.values().iter()
            .rev()
            .cycle();

        iter.find(|value| value.nick() == nick);

        iter.next()
            .expect("Could not get 'EnumValue'")
            .nick()
            .to_string()
    }

    fn next_nick(nick: &str) -> String {
        let enum_class = Self::enum_class();

        let mut iter = enum_class.values().iter()
            .cycle();

        iter.find(|value| value.nick() == nick);

        iter.next()
            .expect("Could not get 'EnumValue'")
            .nick()
            .to_string()
    }
}
