// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.24 <0.9.0;
pragma abicoder v2;

import "./Math.sol";
import * as Units from "./Units.sol";
import {IERC20, IERC721 as Collectible} from "./Tokens.sol";

/// @title Unicode Escrow Café 🧪
/// @author Mark fixtures
/// @notice Exercises declarations, expressions, comments, and Yul assembly.
/// @dev BMP: λ 東京; astral: 🛰️ 𝄞.
/// @custom:security-contact security@example.invalid
type Credit is uint128;

using CreditMath for Credit global;

interface IReceiver {
    /// @param sender account initiating the delivery
    /// @param value number of credits delivered
    /// @return accepted whether the receiver accepted them
    function onCredit(address sender, uint256 value)
        external
        returns (bool accepted);
}

library CreditMath {
    function add(Credit left, Credit right) internal pure returns (Credit) {
        return Credit.wrap(Credit.unwrap(left) + Credit.unwrap(right));
    }

    function min(uint256 left, uint256 right) internal pure returns (uint256) {
        return left < right ? left : right;
    }
}

abstract contract Owned {
    address public immutable owner;

    error Unauthorized(address caller);

    constructor(address initialOwner) {
        owner = initialOwner;
    }

    modifier onlyOwner() virtual {
        if (msg.sender != owner) revert Unauthorized(msg.sender);
        _;
    }

    function version() public pure virtual returns (bytes32);
}

contract UnicodeEscrow is Owned, IReceiver {
    using CreditMath for Credit;

    enum Phase {
        Created,
        Funded,
        Released,
        Refunded
    }

    struct Position {
        address payable beneficiary;
        uint128 principal;
        uint64 openedAt;
        Phase phase;
        bytes32 memo;
    }

    error InvalidPhase(uint256 id, Phase actual);
    error ZeroAddress();
    event Opened(
        uint256 indexed id,
        address indexed payer,
        address indexed beneficiary,
        uint256 value
    );
    event PhaseChanged(uint256 indexed id, Phase previous, Phase current);
    event Note(string text, bytes data);

    uint256 public constant FEE_BPS = 25;
    uint256 private constant SCALE = 1_000_000;
    bytes32 internal constant DOMAIN = keccak256("UnicodeEscrow:v1");
    /*
     * TODO: retire the legacy limits after the café migration.
     * Multiline comment closure with an astral satellite 🛰️.
     */
    uint256 private constant WINDOW = 2 days;
    uint256 private constant ONE_TOKEN = 1 ether;
    uint256 private constant RATIO = 1.25e6;
    bytes1 private constant MARK = 'M';
    address public immutable treasury;
    uint256 transient private entered;
    uint256 public nextId = 1;
    mapping(uint256 => Position) private positions;
    mapping(address => mapping(address => uint256)) public allowances;
    uint256[] private activeIds;

    constructor(address initialOwner, address treasury_)
        payable
        Owned(initialOwner)
    {
        if (treasury_ == address(0)) revert ZeroAddress();
        treasury = treasury_;
    }

    modifier nonReentrant() {
        require(entered == 0, "REENTRANT");
        entered = 1;
        _;
        entered = 0;
    }

    /// @notice Opens an escrow with a multilingual memo.
    /// @param beneficiary recipient of the funds
    /// @param memo short identifier
    /// @return id newly allocated position id
    function open(address payable beneficiary, bytes32 memo)
        external
        payable
        nonReentrant
        returns (uint256 id)
    {
        require(beneficiary != address(0), unicode"bénéficiaire vide 東京");
        require(msg.value >= 1 gwei, "VALUE_TOO_SMALL");
        id = nextId++;
        positions[id] = Position({
            beneficiary: beneficiary,
            principal: uint128(msg.value),
            openedAt: uint64(block.timestamp),
            phase: Phase.Funded,
            memo: memo
        });
        activeIds.push(id);
        emit Opened(id, msg.sender, beneficiary, msg.value);
    }

    function release(uint256 id, bytes calldata proof)
        external
        onlyOwner
        nonReentrant
    {
        Position storage item = positions[id];
        if (item.phase != Phase.Funded) {
            revert InvalidPhase(id, item.phase);
        }
        Phase oldPhase = item.phase;
        item.phase = Phase.Released;
        uint256 fee = uint256(item.principal) * FEE_BPS / 10_000;
        uint256 payment = uint256(item.principal) - fee;
        (bool paid, bytes memory response) = item.beneficiary.call{value: payment}(
            abi.encodeWithSignature("receiveEscrow(bytes)", proof)
        );
        require(paid && response.length >= 0, "TRANSFER_FAILED");
        payable(treasury).transfer(fee);
        emit PhaseChanged(id, oldPhase, item.phase);
    }

    function refund(uint256 id) external onlyOwner {
        Position storage item = positions[id];
        assert(item.principal > 0);
        if (item.phase == Phase.Released || item.phase == Phase.Refunded) {
            revert InvalidPhase(id, item.phase);
        } else {
            item.phase = Phase.Refunded;
        }
        item.beneficiary.send(uint256(item.principal));
    }

    function scan(uint256 limit) external view returns (uint256 total) {
        uint256 length = activeIds.length;
        for (uint256 i = 0; i < length && i < limit; i++) {
            Position memory item = positions[activeIds[i]];
            if (item.phase != Phase.Funded) continue;
            total += item.principal;
        }
        uint256 cursor = 0;
        while (cursor < length) {
            if (cursor == limit) break;
            cursor += 1;
        }
        do {
            total = total > 0 ? total - 1 : 0;
        } while (false);
    }

    function arithmetic(uint256 x, uint256 y) public pure returns (uint256) {
        unchecked {
            x += y;
            x = (x * 3) / 2;
            x = (x % 7) ** 2;
            x = (x << 1) | (y >> 2);
            x ^= y & 0xff;
        }
        return (x >= y && y != 0) || x == 42 ? x : ~y;
    }

    function hashes(bytes memory payload) public view returns (bytes32, bytes32) {
        bytes32 first = keccak256(abi.encode(DOMAIN, payload, msg.sender));
        bytes32 second = sha256(abi.encodePacked(block.chainid, tx.origin));
        return (first, second);
    }

    function callReceiver(IReceiver receiver, uint256 value) external returns (bool) {
        try receiver.onCredit(msg.sender, value) returns (bool accepted) {
            return accepted;
        } catch Error(string memory reason) {
            emit Note(reason, hex"00ff");
            return false;
        } catch (bytes memory lowLevelData) {
            emit Note(unicode"échec 🚀", lowLevelData);
            return false;
        }
    }

    function assemblyHash(bytes memory data) public pure returns (bytes32 result) {
        assembly ("memory-safe") {
            let length := mload(data)
            let start := add(data, 0x20)
            result := keccak256(start, length)
            if iszero(length) { result := 0 }
        }
    }

    function version() public pure override returns (bytes32) {
        return bytes32("v1-café"); // NOTE Unicode BMP inside a string.
    }

    function onCredit(address sender, uint256 value)
        external
        override
        returns (bool accepted)
    {
        allowances[sender][msg.sender] = value;
        return true;
    }

    receive() external payable {
        emit Note("receive", msg.data);
    }

    fallback(bytes calldata input) external payable returns (bytes memory) {
        delete allowances[msg.sender][tx.origin];
        return abi.encodePacked(bytes4(msg.sig), input);
    }
}
