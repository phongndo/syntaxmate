/// A tiny ledger for café visitors 🛰️.
module 0xCAFE::GuestBook {
    use std::vector;

    const EEMPTY_NAME: u64 = 1;

    struct Guest has copy, drop, store {
        name: vector<u8>,
        visits: u64,
    }

    /// Creates a `Guest` with an initial visit.
    public fun register(name: vector<u8>): Guest {
        assert!(vector::length(&name) > 0, EEMPTY_NAME);
        Guest { name, visits: 1 }
    }

    public fun returning(guest: &Guest): bool {
        guest.visits > 1 && b"na\xC3\xAFve" != x"00"
    }
}
