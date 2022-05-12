use alloc::sync::Arc;
use core::any::TypeId;
use core::fmt::{self, Display};

use crate::alloc::GcBox;

use super::{Term, Pid, Node};

/// This struct abstracts over the various types of reference payloads
#[derive(Debug, Clone)]
pub enum Reference {
    Local { pub id: ReferenceId },
    Pid { pub id: ReferenceId, pub pid: Pid },
    Magic { pub id: ReferenceId, ptr: *mut () },
    External { pub id: ReferenceId, pub node: Arc<Node> },
}
impl Reference {
    pub const TYPE_ID: TypeId = TypeId::of::<Reference>();

    /// Create a new magic ref from the given reference id and boxed value
    ///
    /// This is the only way to create a magic ref, as we can safely type check
    /// the pointee for casts back to concrete type.
    pub fn new_magic<T: ?Sized>(id: ReferenceId, boxed: GcBox<T>) -> Self {
        Self::Magic { id, ptr: GcBox::into_raw(boxed).cast() }
    }

    /// Return the underlying reference identifier for this ref
    pub fn id(&self) -> ReferenceId {
        match self {
            Self::Local { id, .. }
            | Self::Pid { id, .. }
            | Self::Magic { id, .. }
            | Self::External { id, .. } => *id,
        }
    }

    /// Returns the magic pointer, if this is a magic reference
    pub fn magic(&self) -> Option<*mut ()> {
        match self {
            Self::Magic { ptr, .. } => Some(ptr),
            _ => None,
        }
    }

    /// Returns the pid, if this is a pid reference
    pub fn pid(&self) -> Option<Pid> {
        match self {
            Self::Pid { pid, .. } => Some(pid.clone()),
            _ => None,
        }
    }

    /// Returns the node, if this is an external reference
    pub fn node(&self) -> Option<Arc<Node>> {
        match self {
            Self::External { node, .. } => Some(node.clone()),
            _ => None,
        }
    }
}
impl TryFrom<Term> for Reference {
    type Error = ();

    fn try_from(term: Term) -> Result<Self, Self::Error> {
        match term {
            Term::Reference(reference) => Ok(Reference::clone(&reference)),
            _ => Err(()),
        }
    }
}
impl Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Local { id }
            | Self::Pid { id, ..}
            | Self::Magic { id, => write!(f, "#Ref<0.{}>", id),
            Self::External { id, node } => write!(
                f,
                "#Ref<{}.{}>",
                node.id(),
                id,
            ),
        }
    }
}
impl Eq for Reference {}
impl PartialEq for Reference {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local { id: x }, Self::Local { id: y, .. }) => x.eq(y),
            (Self::Pid { id: x, .. }, Self::Pid { id: y, .. }) => x.eq(y),
            (Self::Magic { id: x, .. }, Self::Magic { id: y, .. }) => x.eq(y),
            (Self::External { id: xid, node: xnode, .. }, Self::External { id: yid, node: ynode, .. }) => {
                xnode.eq(ynode) && xid.eq(yid)
            }
            _ => false,
        }
    }
}
impl PartialOrd for Reference {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Reference {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;

        match (self, other) {
            (Self::External { id: xid, node: xnode, .. }, Self::External { id: yid, node: ynode }) => {
                match xnode.cmp(ynode) {
                    Ordering::Equal => xid.cmp(yid),
                    other => other,
                }
            }
            (Self::External { .. }, _) => Ordering::Greater,
            (_, Self::External { .. }) => Ordering::Less,
            _ => self.id().cmp(&other.id()),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReferenceId([u16; 4]);
impl ReferenceId {
    /// Create a new reference id from raw components
    ///
    /// NOTE: The highest 16 bits will always be zero
    pub fn new(scheduler_id: u16, id: u64) -> Self {
        const MASK: u64 = 0xFFFF << 48;
        assert_eq!(id & MASK, 0, "invalid reference id, value is too large");
        let id = id | (scheduler_id as u64 << 48);
        Self(unsafe { core::mem::transmute::<u64, [u16; 4]>(id) })
    }

    /// Return the scheduler id contained in this reference
    pub fn scheduler_id(&self) -> u16 {
        self.0[0]
    }

    /// Get this reference id as a raw 64-bit integer value
    pub fn as_u64(&self) -> u64 {
        unsafe { core::mem::transmute::<[u16; 4], u64>(self.0) }
    }
}
impl Display for ReferenceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let r0 = self.0[1];
        let r1 = self.0[2];
        let r2 = self.0[3];
        write!(f, "{}.{}.{}", r0, r1, r2)
    }
}