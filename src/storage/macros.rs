#[macro_export]
macro_rules! storage_objs {
    {
        $($name:ident {
            $($(#[$field_meta:meta])* $field:ident : $field_ty:ty),+ $(,)?
        })+
    } => {
        $(
            paste::paste! {
                #[derive(Clone, Debug, Serialize, Deserialize)]
                pub struct [<Proto $name:camel>] {
                    pub typ: String,
                    #[serde(flatten)]
                    pub props: Attrs,
                    $(pub $field: $field_ty),+
                }

                // TODO explore how typed log can be passed directly
                #[derive(Clone, Debug)]
                #[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
                pub struct [<Script $name:camel>] {
                    pub id: [<$name:camel Id>],
                    pub typ: String,
                    pub props: Attrs,
                    $(pub $field: $field_ty),+
                }

                #[derive(Clone, Debug, Serialize, Deserialize)]
                pub struct [<Raw $name:camel>]<T: [<Api $name:camel>]> {
                    #[serde(flatten)]
                    pub inner: T,
                    pub typ: String,
                    $($(#[$field_meta])* pub $field: $field_ty),+
                }

                #[derive(Clone, Debug)]
                pub struct [<$name:camel>]<T: [<Api $name:camel>]> {
                    pub id: [<$name:camel Id>],
                    pub inner: T,
                    $(pub $field: $field_ty),+
                }

                // #[derive(Clone, Debug, VmType, Pushable, Getable)]
                // pub struct [<Typed $name>]<T> {
                //     pub id: [<$name Id>],
                //     pub typ: String,
                //     pub inner: T,
                //     $(pub $field: $field_ty),+
                // }

                impl<T: [<Api $name:camel>]> [<Raw $name:camel>]<T> {
                    // pub fn with_id_typed(self, id: [<$name Id>]) -> [<Typed $name>]<T> {
                    //     $name {
                    //         id,
                    //         inner: self.inner,
                    //         typ: self.typ,
                    //         $($field: self.$field),+
                    //     }
                    // }

                    pub fn with_id(self, id: [<$name:camel Id>]) -> [<$name:camel>]<T> {
                        [<$name:camel>] {
                            id,
                            inner: self.inner,
                            $($field: self.$field),+
                        }
                    }
                }

                impl<T: [<Api $name:camel>]> From<[<$name:camel>]<T>> for [<Raw $name:camel>]<T> {
                    fn from(normal: [<$name:camel>]<T>) -> Self {
                        Self {
                            typ: T::[<$name:upper _TYPE>].into(),
                            inner: normal.inner,
                            $($field: normal.$field),+
                        }
                    }
                }

                impl [<Proto $name:camel>] {
                    pub fn with_id(self, id: [<$name:camel Id>]) -> [<Script $name:camel>] {
                        [<Script $name:camel>] {
                            id,
                            typ: self.typ,
                            props: self.props,
                            $($field: self.$field),+
                        }
                    }
                }

                // impl<T> From<$name<T>> for [<Typed $name>]<T> {
                //     fn from(normal: $name) -> Self {
                //         Self {
                //             typ: normal.typ,
                //             $($field: normal.$field),+
                //         }
                //     }
                // }
            }
        )+
    }
}

// TODO Optimize string and map to Option<T> to avoid allocation when empty
// FIXME Compact `serde(default)` and `new(default)` in custom attribute?
#[macro_export]
macro_rules! api_objs {
    {
        $($name:ident$(<$lif:lifetime>)? $tag:literal {
            $($(#[$field_meta:meta])* $field:ident : $field_ty:ty),+ $(,)?
        })+
    } => {
        $(
            paste::paste! {
                #[derive(Clone, Debug, Serialize, Deserialize, new)]
                #[serde(rename_all = "kebab-case")]
                pub struct $name$(<$lif>)? {
                    $($(#[$field_meta])* pub $field: $field_ty),+
                }

                impl ApiObj for $name {
                    const OBJ_TYPE: &'static str = $tag;
                }
            }
        )+
    }
}

#[macro_export]
macro_rules! api_logs {
    {
        $($name:ident$(<$lif:lifetime>)? $tag:literal {
            $($(#[$field_meta:meta])* $field:ident : $field_ty:ty),+ $(,)?
        })+
    } => {
        $(
            paste::paste! {
                #[derive(Clone, Debug, Serialize, Deserialize, new)]
                #[cfg_attr(features = "scripting", derive(Trace, VmType, Userdata))]
                #[serde(rename_all = "kebab-case")]
                pub struct $name$(<$lif>)? {
                    $($(#[$field_meta])* pub $field: $field_ty),+
                }

                impl$(<$lif>)? ApiLog for $name$(<$lif>)? {
                    const LOG_TYPE: &'static str = $tag;
                }
            }
        )+
    }
}
