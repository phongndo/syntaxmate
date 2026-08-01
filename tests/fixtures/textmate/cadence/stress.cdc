#!/usr/bin/env cadence
/// Broad Cadence fixture: declarations, types, expressions, and phases.
// Unicode payload: café λ 東京 🚀 𝌆
import Crypto
import FungibleToken, MetadataViews from 0x0000000000000001
import Example from "Example.cdc"

/**
 * Documentation spans lines and contains nested state.
 * café λ 東京 🚀 𝌆
 * /* nested block comment, also fully closed */
 */
/*:
 Playground documentation with punctuation: {} [] () "quoted".
 /* a second nested comment */
*/

entitlement Withdraw
entitlement Deposit
entitlement Admin
entitlement mapping VaultAccess {
    Withdraw -> Withdraw
    Deposit -> Deposit
    Admin -> Withdraw
    Admin -> Deposit
}

access(all) enum Status: UInt8 {
    access(all) case idle
    access(all) case active
    access(all) case closed
}

access(all) struct interface Named {
    access(all) let name: String
    access(all) view fun label(): String
}

access(all) struct Profile: Named {
    access(all) let name: String
    access(all) var tags: [String]
    access(all) var scores: {String: Int}
    access(all) let callback: view fun(String): String

    init(name: String, tags: [String]) {
        self.name = name
        self.tags = tags
        self.scores = {}
        self.callback = view fun (_ value: String): String {
            return "profile=\(value)"
        }
    }

    access(all) view fun label(): String {
        pre {
            self.name.length > 0: "name must not be empty"
        }
        post {
            result.length > 0: "label must not be empty"
        }
        return "Profile(\(self.name))"
    }

    access(all) fun update(
        tag newTag: String,
        score: Int
    ) {
        self.tags.append(newTag)
        self.scores[newTag] = score
    }
}

access(all) event Deposited(
    id: UInt64,
    amount: UFix64,
    memo: String
)
access(all) event Withdrawn(id: UInt64, amount: UFix64)
access(all) resource interface Balance {
    access(all) var balance: UFix64
    access(Deposit) fun deposit(from: @Vault)
}

access(all) resource Vault: Balance {
    access(all) let id: UInt64
    access(all) var balance: UFix64
    access(self) var history: [UFix64]

    init(id: UInt64, balance: UFix64) {
        self.id = id
        self.balance = balance
        self.history = []
    }

    access(Deposit) fun deposit(from: @Vault) {
        let incoming <- from
        self.balance = self.balance + incoming.balance
        self.history.append(incoming.balance)
        emit Deposited(
            id: self.id,
            amount: incoming.balance,
            memo: "café deposit 🚀"
        )
        destroy incoming
    }

    access(Withdraw) fun withdraw(amount: UFix64): @Vault {
        pre {
            amount > 0.0: "positive amount required"
            amount <= self.balance: "insufficient balance"
        }
        self.balance = self.balance - amount
        emit Withdrawn(id: self.id, amount: amount)
        return <-create Vault(id: self.id, balance: amount)
    }

    access(all) view fun describe(): String {
        return "Vault #\(self.id): \(self.balance) 東京"
    }
}

access(all) attachment Audit for Vault {
    access(all) let createdAt: UInt64

    init(createdAt: UInt64) {
        self.createdAt = createdAt
    }

    access(all) view fun summary(): String {
        return "audit=\(self.createdAt) 𝌆"
    }
}

access(all) contract Treasury {
    access(all) let publicPath: PublicPath
    access(all) let storagePath: StoragePath
    access(all) var status: Status
    access(account) var serial: UInt64

    init() {
        self.publicPath = /public/vault
        self.storagePath = /storage/vault
        self.status = Status.idle
        self.serial = 0b0001_0010 + 0o17 + 0xCA_FE + 1_000
    }

    access(all) fun choose(_ left: Int?, _ right: Int?): Int {
        let first = left ?? 0
        let second = right ?? 0
        if first >= second || second == 0 {
            return first
        }
        return second
    }

    access(all) fun classify(_ value: Int): String {
        switch value {
        case 0:
            return "zero"
        case 1:
            return "one"
        default:
            return "many"
        }
    }

    access(all) fun loops(values: [Int]): Int {
        var total = 0
        var index = 0
        while index < values.length {
            if values[index] < 0 {
                index = index + 1
                continue
            }
            total = total + values[index]
            if total > 10_000 {
                break
            }
            index = index + 1
        }
        for value in values {
            total = total + value
        }
        return total
    }

    access(all) fun typeSamples(
        reference: auth(Withdraw | Deposit) &Vault,
        mapped: auth(mapping VaultAccess) &Vault,
        intersection: &{Balance},
        matrix: [[Int]],
        lookup: {String: [Int?]}
    ): Type {
        let maybeVault = reference as? &Vault
        let forcedVault = mapped as! &Vault
        let anyValue: AnyStruct = matrix
        let checked = anyValue as [AnyStruct]
        return Type<@Vault>()
    }

    access(all) fun operators(a: Int, b: Int): Bool {
        var x = a
        var y = b
        x <-> y
        let shifted = (x << 2) + (y >> 1)
        let remainder = shifted % 3
        return !(remainder != 0) && x <= y
    }
}

access(all) fun genericCalls(): String {
    let type = Type<Profile>()
    let value = Treasury.choose(nil, 42)
    return "type=\(type), value=\(value), escape=\n\t\"\u{1F680}"
}
transaction(name: String, amount: UFix64) {
    let signerAddress: Address
    prepare(signer: auth(Storage, Capabilities) &Account) {
        self.signerAddress = signer.address
        let existing = signer.storage.borrow<&Vault>(from: /storage/vault)
        if existing == nil {
            signer.storage.save(
                <-create Vault(id: 1, balance: amount),
                to: /storage/vault
            )
        }
    }
    pre {
        amount > 0.0: "amount must be positive"
    }
    execute {
        let greeting = "Hello \(name), café λ 東京 🚀 𝌆"
        log(greeting)
    }
    post {
        self.signerAddress != 0x0: "signer was recorded"
    }
}
