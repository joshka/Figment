use std::panic::Location;

use serde::Serialize;

use crate::{Profile, Provider, Metadata};
use crate::error::{Error, Kind::InvalidType};
use crate::value::{Value, Map, Dict};

/// A `Provider` that sources values directly from a serialize type.
///
/// # Provider Details
///
///   * **Profile**
///
///     This provider does not set a profile.
///
///   * **Metadata**
///
///     This provider is named `T` (via [`std::any::type_name`]). The source
///     location is set to the call site of the constructor.
///
///   * **Data (Unkeyed)**
///
///     When data is not keyed, `T` is expected to serialize to a [`Dict`] and
///     is emitted directly as the value for the configured profile.
///
///   * **Data (Keyed)**
///
///     When keyed, the `T` can serialize as any [`Value`] and is emitted as the
///     value of the configured `key`. Specifically, nested dictionaries are
///     created for every path component delimited by `.` in the key string (3
///     in `a.b.c`), each dictionary mapping to its parent, and the value
///     mapping to the leaf.
#[derive(Debug, Clone)]
pub struct Serialized<T> {
    /// The value to be serialized and used as the provided data.
    pub value: T,
    /// The key path (`a.b.c`) to emit the value to or the root if `None`.
    pub key: Option<String>,
    /// The profile to emit the value to. Defaults to [`Profile::Default`].
    pub profile: Profile,
    loc: &'static Location<'static>,
}

impl<T> Serialized<T> {
    /// Constructs an (unkeyed) provider that emits `value` to the `profile`.
    ///
    /// ```rust
    /// use serde::Deserialize;
    /// use figment::{Figment, Jail, providers::Serialized, util::map};
    ///
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct Config {
    ///     numbers: Vec<usize>,
    /// }
    ///
    /// Jail::expect_with(|jail| {
    ///     let map = map!["numbers" => &[1, 2, 3]];
    ///
    ///     // This is also `Serialized::defaults(&map)`;
    ///     let figment = Figment::from(Serialized::from(&map, "default"));
    ///     let config: Config = figment.extract()?;
    ///     assert_eq!(config, Config { numbers: vec![1, 2, 3] });
    ///
    ///     // This is also `Serialized::defaults(&map).profile("debug")`;
    ///     let figment = Figment::from(Serialized::from(&map, "debug"));
    ///     let config: Config = figment.select("debug").extract()?;
    ///     assert_eq!(config, Config { numbers: vec![1, 2, 3] });
    ///
    ///     Ok(())
    /// });
    /// ```
    #[track_caller]
    pub fn from<P: Into<Profile>>(value: T, profile: P) -> Serialized<T> {
        Serialized {
            value,
            key: None,
            profile: profile.into(),
            loc: Location::caller()
        }
    }

    /// Emits `value`, which must serialize to a [`Dict`], to the `Default`
    /// profile.
    ///
    /// Equivalent to `Serialized::from(value, Profile::Default)`.
    ///
    /// See [`Serialized::from()`].
    #[track_caller]
    pub fn defaults(value: T) -> Serialized<T> {
        Self::from(value, Profile::Default)
    }

    /// Emits `value`, which must serialize to a [`Dict`], to the `Global`
    /// profile.
    ///
    /// Equivalent to `Serialized::from(value, Profile::Global)`.
    ///
    /// See [`Serialized::from()`].
    #[track_caller]
    pub fn globals(value: T) -> Serialized<T> {
        Self::from(value, Profile::Global)
    }

    /// Emits a nested dictionary to the `Default` profile keyed by `key` with
    /// the final key mapping to `value`.
    ///
    /// Equivalent for `Serialized::from(value, Profile::Default).key(key)`.
    ///
    /// See [`Serialized::from()`] and [`Serialized::key()`].
    #[track_caller]
    pub fn default(key: &str, value: T) -> Serialized<T> {
        Self::from(value, Profile::Default).key(key)
    }

    /// Emits a nested dictionary to the `Global` profile keyed by `key` with
    /// the final key mapping to `value`.
    ///
    /// Equivalent to `Serialized::from(value, Profile::Global).key(key)`.
    ///
    /// See [`Serialized::from()`] and [`Serialized::key()`].
    #[track_caller]
    pub fn global(key: &str, value: T) -> Serialized<T> {
        Self::from(value, Profile::Global).key(key)
    }

    pub fn profile<P: Into<Profile>>(mut self, profile: P) -> Self {
        self.profile = profile.into();
        self
    }

    pub fn key(mut self, key: &str) -> Self {
        self.key = Some(key.into());
        self
    }
}

fn value_from(mut keys: std::str::Split<'_, char>, value: Value) -> Value {
    match keys.next() {
        Some(k) if !k.is_empty() => {
            let mut dict = Dict::new();
            dict.insert(k.into(), value_from(keys, value));
            dict.into()
        }
        Some(_) | None => value
    }
}

impl<T: Serialize> Provider for Serialized<T> {
    fn metadata(&self) -> Metadata {
        Metadata::from(std::any::type_name::<T>(), self.loc)
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        let value = Value::serialize(&self.value)?;
        let error = InvalidType(value.to_actual(), "map".into());
        let dict = match &self.key {
            Some(key) => value_from(key.split('.'), value).into_dict().ok_or(error)?,
            None => value.into_dict().ok_or(error)?,
        };

        Ok(self.profile.clone().collect(dict))
    }
}