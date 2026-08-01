/*
 * Observatory registry fixture.
 * Tracks café instruments from Zürich to orbit 🪐.
 */
address 0xCAFE {
module Observatory {
    use std::option::{Self, Option};
    use std::signer;
    use std::string::{Self as string, String};
    use std::vector;
    use fun std::vector::length as vector_length;

    friend 0xCAFE::Calibrator;

    const ENOT_AUTHORIZED: u64 = 1;
    const ENOT_FOUND: u64 = 2u64;
    const EINVALID_READING: u64 = 0x0_3u64;
    const DEFAULT_LIMIT: u16 = 1_024u16;
    const HOME: address = @0xCAFE;

    /// Marker for the owner of an `Archive`.
    struct Curator has drop {}

    /// A calibrated scalar with phantom unit information.
    struct Reading<phantom Unit> has copy, drop, store {
        value: u128,
        uncertainty: u64,
        accepted: bool,
    }

    struct Instrument has key, store {
        id: UID,
        serial: vector<u8>,
        samples: vector<u128>,
        note: String,
    }

    enum Command has copy, drop, store {
        Pause,
        Resume(u64),
        Rename { bytes: vector<u8> },
    }

    enum Status<T: copy + drop> has copy, drop {
        Missing,
        Ready(T),
        Fault { code: u64, retry: bool },
    }

    #[test_only]
    public fun synthetic_reading(): Reading<Curator> {
        Reading<Curator> {
            value: 6_022_140_76u128,
            uncertainty: 12,
            accepted: true,
        }
    }

    #[allow(lint(share_owned))]
    public entry fun publish(
        account: &signer,
        serial: vector<u8>,
        samples: vector<u128>,
        note: vector<u8>,
    ) {
        let owner = signer::address_of(account);
        assert!(owner == HOME, ENOT_AUTHORIZED);
        assert!(vector_length(&samples) <= (DEFAULT_LIMIT as u64), EINVALID_READING);
        let instrument = Instrument {
            id: object::new(account),
            serial,
            samples,
            note: string::utf8(note),
        };
        transfer::share_object(instrument);
    }

    public fun classify(value: u64): Status<u64> {
        if (value == 0) {
            Status::Missing
        } else if (value < 100) {
            Status::Ready(value)
        } else {
            Status::Fault { code: value, retry: false }
        }
    }

    public fun apply_command(inst: &mut Instrument, command: Command) {
        match (command) {
            Command::Pause => vector::push_back(&mut inst.samples, 0),
            Command::Resume(offset) => {
                vector::push_back(&mut inst.samples, offset as u128);
            },
            Command::Rename { bytes } => {
                inst.serial = bytes;
            },
        };
    }

    public fun summarize(inst: &Instrument): (u128, u64) {
        let mut total = 0u128;
        let mut index = 0;
        while (index < vector::length(&inst.samples)) {
            total = total + *vector::borrow(&inst.samples, index);
            index = index + 1;
        };
        (total, index)
    }

    public fun first_nonzero(values: &vector<u128>): Option<u128> {
        let mut cursor = 0;
        'search: loop {
            if (cursor >= vector::length(values)) {
                break 'search option::none()
            };
            let candidate = *vector::borrow(values, cursor);
            if (candidate != 0) {
                break 'search option::some(candidate)
            };
            cursor = cursor + 1;
            continue
        }
    }

    public fun literal_gallery(): (vector<u8>, vector<u8>, vector<u64>) {
        let greeting = b"caf\xC3\xA9 \"station\" \\ orbit";
        let packet = x"CAFE00ff10";
        let primes = vector[2u64, 3, 5, 7, 0x0B];
        (greeting, packet, primes)
    }

    public fun addresses(): (address, address) {
        (@0xCAFE, @Observatory)
    }

    public fun resource_probe(owner: address): bool {
        if (exists<Instrument>(owner)) {
            let item = borrow_global<Instrument>(owner);
            vector::length(&item.samples) > 0
        } else {
            false
        }
    }

    public(friend) fun take_for_service(owner: address): Instrument acquires Instrument {
        move_from<Instrument>(owner)
    }

    public(package) fun copy_sample(reading: &Reading<Curator>): u128 {
        let alias = copy reading.value;
        alias
    }

    inline fun consume_sample(sample: Reading<Curator>): u128 {
        let Reading { value, uncertainty: _, accepted: _ } = move sample;
        value
    }

    native fun hardware_clock(): u64;

    macro fun repeat_sample<$action: drop>($count: u64) {
        let mut n = 0;
        while (n < $count) {
            $action;
            n = n + 1;
        }
    }

    public fun run_macro(samples: &mut vector<u64>) {
        repeat_sample!(vector::push_back(samples, 42), 3);
        samples.reverse!();
    }

    /* Block comments may contain punctuation: { <T> @0x1 }.
       The grammar keeps this multiline state closed. */
    public fun unicode_note(): vector<u8> {
        // BMP: λ, Ж, 中; astral: 🚀, 𝄞.
        b"Unicode is documented in the adjacent comment"
    }

    spec module {
        pragma verify = true;
        invariant DEFAULT_LIMIT > 0;
    }

    spec publish {
        requires signer::address_of(account) == HOME;
        aborts_if vector::length(&samples) > (DEFAULT_LIMIT as u64);
        ensures true;
    }

    spec fun classify(value: u64): Status<u64> {
        ensures value == 0 ==> result == Status::Missing;
    }
}
}

script {
    use 0xCAFE::Observatory;

    fun main(account: &signer) {
        let reading = Observatory::synthetic_reading();
        let _value = reading.value;
    }
}
