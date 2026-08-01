# Compact Vyper contract fixture: café λ 東京 🚀 𝌆.
event Greeting:
    sender: indexed(address)
    message: String[96]

struct Visit:
    count: uint256
    active: bool

OWNER: immutable(address)
visits: public(HashMap[address, Visit])

@deploy
def __init__():
    OWNER = msg.sender

@external
def greet(name: String[32], count: uint256 = 1) -> String[96]:
    """Return a closed Unicode greeting, including 🚀 and 𝌆."""
    assert count > 0, "count must be positive"
    self.visits[msg.sender] = Visit(count=count, active=True)
    message: String[96] = concat("Hello, ", name, " — café λ 東京 🚀 𝌆")
    log Greeting(sender=msg.sender, message=message)
    return message

@external
@view
def visit_count(account: address) -> uint256:
    return self.visits[account].count
