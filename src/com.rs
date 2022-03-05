use crate::D3DResult;
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    ptr,
};
use winapi::{ctypes::c_void, um::unknwnbase::IUnknown, Interface};

#[repr(transparent)]
pub struct WeakPtr<T>(*mut T);

impl<T> WeakPtr<T> {
    pub fn null() -> Self {
        WeakPtr(ptr::null_mut())
    }

    pub unsafe fn from_raw(raw: *mut T) -> Self {
        WeakPtr(raw)
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn as_ptr(&self) -> *const T {
        self.0
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        self.0
    }

    pub fn mut_void(&mut self) -> *mut *mut c_void {
        &mut self.0 as *mut *mut _ as *mut *mut _
    }

    pub fn mut_self(&mut self) -> *mut *mut T {
        &mut self.0 as *mut *mut _
    }
}

impl<T: Interface> WeakPtr<T> {
    pub unsafe fn as_unknown(&self) -> &IUnknown {
        debug_assert!(!self.is_null());
        &*(self.0 as *mut IUnknown)
    }

    // Cast creates a new WeakPtr requiring explicit destroy call.
    pub unsafe fn cast<U>(&self) -> D3DResult<WeakPtr<U>>
    where
        U: Interface,
    {
        let mut obj = WeakPtr::<U>::null();
        let hr = self
            .as_unknown()
            .QueryInterface(&U::uuidof(), obj.mut_void());
        (obj, hr)
    }

    // Destroying one instance of the WeakPtr will invalidate all
    // copies and clones.
    pub unsafe fn destroy(&self) {
        self.as_unknown().Release();
    }
}

impl<T> Clone for WeakPtr<T> {
    fn clone(&self) -> Self {
        WeakPtr(self.0)
    }
}

impl<T> Copy for WeakPtr<T> {}

impl<T> Deref for WeakPtr<T> {
    type Target = T;
    fn deref(&self) -> &T {
        debug_assert!(!self.is_null());
        unsafe { &*self.0 }
    }
}

impl<T> fmt::Debug for WeakPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "WeakPtr( ptr: {:?} )", self.0)
    }
}

impl<T> PartialEq<*mut T> for WeakPtr<T> {
    fn eq(&self, other: &*mut T) -> bool {
        self.0 == *other
    }
}

impl<T> PartialEq for WeakPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Hash for WeakPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Macro that allows generation of an easy to use enum for dealing with many different possible versions of a COM object.
///
/// Give the variants so that parents come before children. This often manifests as going up in order (1 -> 2 -> 3). This is vital for safety.
///
/// Three function names need to be attached to each variant. The examples are given for the MyComObject1 variant below:
/// - the from function (`WeakPtr<actual::ComObject1> -> Self`)
/// - the as function (`&self -> Option<WeakPtr<actual::ComObject1>>`)
/// - the unwrap function (`&self -> WeakPtr<actual::ComObject1>` panicing on failure to cast)
///
/// ```no_compile,rust
/// weak_com_inheritance_chain! {
///     pub enum MyComObject {
///         MyComObject(actual::ComObject), from_my_com_object; as_my_com_object, my_com_object; // First variant doesn't use "unwrap" as it can never fail
///         MyComObject1(actual::ComObject1), from_my_com_object1, as_my_com_object1, unwrap_my_com_object1;
///         MyComObject2(actual::ComObject2), from_my_com_object2, as_my_com_object2, unwrap_my_com_object2;
///     }
/// }
/// ```
#[macro_export]
macro_rules! weak_com_inheritance_chain {
    // We first match a human readable enum style, before going into the recursive section.
    //
    // Internal calls to the macro have either the prefix
    // - @recursion_logic for the recursion and termination
    // - @render_members for the actual call to fill in the members.
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $first_variant:ident($first_type:ty), $first_from_name:ident, $first_as_name:ident, $first_unwrap_name:ident $(;)?
            $($variant:ident($type:ty), $from_name:ident, $as_name:ident, $unwrap_name:ident);* $(;)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $first_variant($crate::WeakPtr<$first_type>),
            $(
                $variant($crate::WeakPtr<$type>)
            ),+
        }
        impl $name {
            $vis unsafe fn destroy(&self) {
                match *self {
                    Self::$first_variant(v) => v.destroy(),
                    $(
                        Self::$variant(v) => v.destroy(),
                    )*
                }
            }

            $crate::weak_com_inheritance_chain! {
                @recursion_logic,
                $vis,
                ;
                $first_variant($first_type), $first_from_name, $first_as_name, $first_unwrap_name;
                $($variant($type), $from_name, $as_name, $unwrap_name);*
            }
        }

        impl std::ops::Deref for $name {
            type Target = $crate::WeakPtr<$first_type>;
            fn deref(&self) -> &Self::Target {
                self.$first_unwrap_name()
            }
        }
    };

    // This is the iteration case of the recursion. We instantiate the member functions for the variant we
    // are currently at, recursing on ourself for the next variant. Note we only keep track of the previous
    // variant name, not the functions names, as those are not needed.
    (
        @recursion_logic,
        $vis:vis,
        $(,)? $($prev_variant:ident),* $(,)?;
        $this_variant:ident($this_type:ty), $this_from_name:ident, $this_as_name:ident, $this_unwrap_name:ident $(;)?
        $($next_variant:ident($next_type:ty), $next_from_name:ident, $next_as_name:ident, $next_unwrap_name:ident);*
    ) => {
        // Actually generate the members for this variant. Needs the previous and future variant names.
        $crate::weak_com_inheritance_chain! {
            @render_members,
            $vis,
            $this_from_name, $this_as_name, $this_unwrap_name;
            $($prev_variant),*;
            $this_variant($this_type);
            $($next_variant),*;
        }

        // Recurse on ourselves. If there is no future variants left, we'll hit the base case as the final expansion returns no tokens.
        $crate::weak_com_inheritance_chain! {
            @recursion_logic,
            $vis,
            $($prev_variant),* , $this_variant;
            $($next_variant($next_type), $next_from_name, $next_as_name, $next_unwrap_name);*
        }
    };
    // Base case for recursion. There are no more variants left
    (
        @recursion_logic,
        $vis:vis,
        $($prev_variant:ident),*;
    ) => {};


    // This is where we generate the members using the given names.
    (
        @render_members,
        $vis:vis,
        $from_name:ident, $as_name:ident, $unwrap_name:ident;
        $($prev_variant:ident),*;
        $variant:ident($type:ty);
        $($next_variant:ident),*;
    ) => {
        // Construct this enum from weak pointer to this interface. For best usability, always use the highest constructor you can. This doesn't try to upcast.
        $vis unsafe fn $from_name(value: $crate::WeakPtr<$type>) -> Self {
            Self::$variant(value)
        }

        // Returns Some if the value implements the interface otherwise returns None.
        $vis fn $as_name(&self) -> Option<&$crate::WeakPtr<$type>> {
            match *self {
                $(
                    Self::$prev_variant(_) => None,
                )*
                Self::$variant(ref v) => Some(v),
                $(
                    Self::$next_variant(ref v) => {
                        // v is &WeakPtr<NextType> and se cast to &WeakPtr<Type>
                        Some(unsafe { std::mem::transmute(v) })
                    }
                )*
            }
        }

        // Returns the interface if the value implements it, otherwise panics.
        #[track_caller]
        $vis fn $unwrap_name(&self) -> &$crate::WeakPtr<$type> {
            match *self {
                $(
                    Self::$prev_variant(_) => panic!(concat!("Tried to unwrap a ", stringify!($prev_variant), " as a ", stringify!($variant))),
                )*
                Self::$variant(ref v) => &*v,
                $(
                    Self::$next_variant(ref v) => {
                        // v is &WeakPtr<NextType> and se cast to &WeakPtr<Type>
                        unsafe { std::mem::transmute(v) }
                    }
                )*
            }
        }
    };
}
