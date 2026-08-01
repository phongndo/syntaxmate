# Escrow-style Vyper grammar fixture with café λ 東京 🚀 and 𝌆.
import ethereum.ercs as ercs
from snekmate.auth import ownable
initializes: ownable
exports: ownable.owner

interface ERC20:
    def transfer(receiver: address, amount: uint256) -> bool: nonpayable
    def transferFrom(sender: address, receiver: address, amount: uint256) -> bool: nonpayable
    def balanceOf(account: address) -> uint256: view

event Deposited:
    deposit_id: indexed(bytes32)
    sender: indexed(address)
    beneficiary: address
    amount: uint256
    memo: String[96]

event Released:
    deposit_id: indexed(bytes32)
    beneficiary: indexed(address)
    amount: uint256

event Refunded:
    deposit_id: indexed(bytes32)
    sender: indexed(address)
    amount: uint256

event GuardianChanged:
    old_guardian: address
    new_guardian: address

struct Deposit:
    sender: address
    beneficiary: address
    token: address
    amount: uint256
    unlock_at: uint256
    released: bool
    memo: String[96]

enum DepositState:
    Missing
    Locked
    Ready
    Released

flag Permission:
    RELEASE
    REFUND
    PAUSE

MAX_MEMO: constant(uint256) = 96
DEFAULT_DELAY: constant(uint256) = 86_400
MAX_BATCH: constant(uint256) = 32
VERSION: constant(String[16]) = "1.2.0"
UNICODE_NOTE: constant(String[64]) = "café λ 東京 🚀 𝌆"
ROUTE_PATTERN: constant(String[64]) = r"^(release|refund)\s+[0-9]+$"
MAGIC_PREFIX: constant(Bytes[8]) = b"VYP\x45\n"
ZERO_ID: constant(bytes32) = empty(bytes32)
OWNER: immutable(address)
TOKEN: immutable(address)

guardian: public(address)
paused: public(bool)
nonce: public(uint256)
total_locked: public(uint256)
deposits: HashMap[bytes32, Deposit]
permissions: HashMap[address, Permission]
beneficiary_ids: HashMap[address, DynArray[bytes32, 32]]

@deploy
def __init__(token: address, guardian: address):
    """Create the escrow.

    The documentation state spans lines and closes here: café λ 東京 🚀 𝌆.
    """
    assert token != empty(address), "token is zero"
    assert guardian != empty(address), "guardian is zero"
    OWNER = msg.sender
    TOKEN = token
    self.guardian = guardian
    self.permissions[guardian] = Permission.RELEASE | Permission.PAUSE

@internal
@view
def _id(sender: address, beneficiary: address, salt: bytes32) -> bytes32:
    payload: Bytes[96] = concat(convert(sender, bytes32), convert(beneficiary, bytes32), salt)
    return keccak256(payload)

@internal
@view
def _state(item: Deposit) -> DepositState:
    if item.sender == empty(address):
        return DepositState.Missing
    elif item.released:
        return DepositState.Released
    elif block.timestamp >= item.unlock_at:
        return DepositState.Ready
    else:
        return DepositState.Locked

@internal
def _require_permission(account: address, needed: Permission):
    granted: Permission = self.permissions[account]
    assert (granted & needed) == needed, "permission denied"

@external
def deposit(beneficiary: address, amount: uint256, salt: bytes32, memo: String[96]) -> bytes32:
    assert not self.paused, "escrow paused"
    assert beneficiary != empty(address), "beneficiary is zero"
    assert amount > 0 and amount <= max_value(uint256), "invalid amount"
    deposit_id: bytes32 = self._id(msg.sender, beneficiary, salt)
    assert deposit_id != ZERO_ID and self.deposits[deposit_id].sender == empty(address), "duplicate"
    success: bool = extcall ERC20(TOKEN).transferFrom(msg.sender, self, amount)
    assert success, "transferFrom failed"
    unlock_at: uint256 = block.timestamp + DEFAULT_DELAY
    self.deposits[deposit_id] = Deposit(
        sender=msg.sender,
        beneficiary=beneficiary,
        token=TOKEN,
        amount=amount,
        unlock_at=unlock_at,
        released=False,
        memo=memo,
    )
    self.beneficiary_ids[beneficiary].append(deposit_id)
    self.total_locked += amount
    self.nonce += 1
    log Deposited(
        deposit_id=deposit_id,
        sender=msg.sender,
        beneficiary=beneficiary,
        amount=amount,
        memo=memo,
    )
    return deposit_id

@external
def release(deposit_id: bytes32):
    item: Deposit = self.deposits[deposit_id]
    allowed: bool = msg.sender == item.beneficiary or msg.sender == self.guardian
    assert allowed, "not beneficiary or guardian"
    assert self._state(item) == DepositState.Ready, "deposit is locked"
    item.released = True
    self.deposits[deposit_id] = item
    self.total_locked -= item.amount
    success: bool = extcall ERC20(item.token).transfer(item.beneficiary, item.amount)
    assert success, "release transfer failed"
    log Released(deposit_id=deposit_id, beneficiary=item.beneficiary, amount=item.amount)

@external
def refund(deposit_id: bytes32):
    item: Deposit = self.deposits[deposit_id]
    assert msg.sender == item.sender, "not depositor"
    assert not item.released, "already released"
    assert block.timestamp < item.unlock_at, "already unlocked"
    item.released = True
    self.deposits[deposit_id] = item
    self.total_locked -= item.amount
    success: bool = extcall ERC20(item.token).transfer(item.sender, item.amount)
    assert success, "refund transfer failed"
    log Refunded(deposit_id=deposit_id, sender=item.sender, amount=item.amount)

@external
@view
def preview(deposit_id: bytes32) -> (DepositState, uint256, String[96]):
    item: Deposit = self.deposits[deposit_id]
    remaining: uint256 = 0
    if block.timestamp < item.unlock_at:
        remaining = item.unlock_at - block.timestamp
    return self._state(item), remaining, item.memo

@external
@view
def batch_states(owner: address) -> DynArray[DepositState, 32]:
    answer: DynArray[DepositState, 32] = []
    ids: DynArray[bytes32, 32] = self.beneficiary_ids[owner]
    for deposit_id: bytes32 in ids:
        answer.append(self._state(self.deposits[deposit_id]))
    return answer

@external
def set_permission(account: address, permission: Permission, enabled: bool):
    assert msg.sender == OWNER, "only owner"
    if enabled:
        self.permissions[account] |= permission
    else:
        self.permissions[account] &= ~permission

@external
def set_guardian(new_guardian: address):
    assert msg.sender == OWNER, "only owner"
    assert new_guardian != empty(address), "guardian is zero"
    old_guardian: address = self.guardian
    self.guardian = new_guardian
    log GuardianChanged(old_guardian=old_guardian, new_guardian=new_guardian)

@external
def set_paused(value: bool):
    self._require_permission(msg.sender, Permission.PAUSE)
    self.paused = value

@external
@view
def token_balance() -> uint256:
    return staticcall ERC20(TOKEN).balanceOf(self)

@external
@pure
def numeric_probe(value: uint256) -> uint256:
    # TODO: number/operator families: decimal, hex, binary, shifts, modulo.
    mask: uint256 = 0xFF
    flags: uint256 = 0b1010
    scaled: uint256 = unsafe_add(value << 1, flags)
    return (scaled & mask) % 1_000

@external
@pure
def decimal_probe(value: decimal = 1.25) -> decimal:
    return value * 2.0 + 0.5

@external
@view
def environment_probe() -> (uint256, uint256, address, bytes32):
    # Special Vyper variables plus an astral comment: orbit 🚀 around 𝌆.
    return block.number, chain.id, tx.origin, block.prevhash
