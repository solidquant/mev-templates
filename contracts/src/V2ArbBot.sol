// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import "openzeppelin-contracts/token/ERC20/IERC20.sol";
import "openzeppelin-contracts/token/ERC20/utils/SafeERC20.sol";

import "./interface/IWETH.sol";
import "./interface/IUniswapV2.sol";
import "./interface/IBalancer.sol";

contract V2ArbBot is IFlashLoanRecipient, IUniswapV2Callee {
    // can perform flashloan, multihop swaps in Uniswap V2 variant pools
    using SafeERC20 for IERC20;

    address public immutable owner;
    IWETH public immutable mainCurrency;

    receive() external payable {
        // wrap on receive
        mainCurrency.deposit{value: msg.value}();
    }

    constructor(address _owner, address _mainCurrency) {
        // mainCurrency on Ethereum is WETH
        // mainCurrency on Polygon is WMATIC
        owner = _owner;
        mainCurrency = IWETH(_mainCurrency);
    }

    function recoverToken(address token) public payable {
        require(msg.sender == owner, "not owner");
        IERC20(token).safeTransfer(
            msg.sender,
            IERC20(token).balanceOf(address(this)) - 1
        );
    }

    function approveRouter(
        address router,
        address[] memory tokens,
        bool force
    ) public {
        // skip approval if it already has allowance and if force is false
        uint maxInt = type(uint256).max;

        uint tokensLength = tokens.length;

        for (uint i; i < tokensLength; ) {
            IERC20 token = IERC20(tokens[i]);
            uint allowance = token.allowance(address(this), router);
            if (allowance < (maxInt / 2) || force) {
                token.safeApprove(router, maxInt);
            }

            unchecked {
                i++;
            }
        }
    }

    function _execute(bytes memory data) internal returns (uint amountOut) {
        uint8 nhop;

        assembly {
            nhop := sub(div(mload(data), 0x60), 1)

            let offset := add(data, 0x20)
            amountOut := mload(offset)
        }

        for (uint8 i; i < nhop; ) {
            address router;
            address tokenIn;
            address tokenOut;

            assembly {
                let offset := add(add(data, 0x20), 0x60)
                offset := add(offset, mul(0x60, i))

                router := mload(offset)
                tokenIn := mload(add(offset, 0x20))
                tokenOut := mload(add(offset, 0x40))
            }

            address[] memory tokens;
            tokens = new address[](2);
            tokens[0] = tokenIn;
            tokens[1] = tokenOut;
            approveRouter(router, tokens, false);

            address[] memory path;
            path = new address[](2);
            path[0] = tokenIn;
            path[1] = tokenOut;

            IUniswapV2Router router2 = IUniswapV2Router(router);
            uint[] memory amounts = router2.swapExactTokensForTokens(
                amountOut,
                0,
                path,
                address(this),
                block.timestamp + 60
            );

            amountOut = amounts[1];

            unchecked {
                i++;
            }
        }
    }

    function receiveFlashLoan(
        IERC20[] memory tokens,
        uint[] memory amounts,
        uint[] memory,
        bytes memory data
    ) external override {
        address vault;

        assembly {
            let offset := add(data, 0x20)
            vault := mload(add(offset, 0x40))
        }

        require(msg.sender == vault, "not vault");

        IERC20 token = tokens[0];
        uint amountIn = amounts[0];

        // we don't need any amountOut checks for this
        // because if we can't pay back the loan, our function simply reverts
        _execute(data);

        // repay the amount borrowed from flashloan
        token.transfer(vault, amountIn);
    }

    function uniswapV2Call(
        address sender,
        uint amount0,
        uint amount1,
        bytes memory data
    ) external override {
        address loanPool;
        address tokenIn;

        assembly {
            let offset := add(data, 0x20)
            loanPool := mload(add(offset, 0x40))
            tokenIn := mload(add(offset, 0x80))
        }

        require(msg.sender == loanPool, "not loanPool");
        require(sender == address(this), "not sender");

        // we don't need any amountOut checks for this
        // because if we can't pay back the loan, our function simply reverts
        _execute(data);

        uint amountIn = amount0 == 0 ? amount1 : amount0;
        uint fee = (amountIn * 3) / 997 + 1;
        uint repayAmount = amountIn + fee;

        // repay the amount borrowed from flashloan: (amount + fee)
        IERC20(tokenIn).transfer(loanPool, repayAmount);
    }

    fallback() external payable {
        uint amountIn;
        uint useLoan;
        address loanPool;

        address _owner = owner;

        assembly {
            // only the owner can call fallback
            if iszero(eq(caller(), _owner)) {
                revert(0, 0)
            }

            amountIn := calldataload(0x00)
            useLoan := calldataload(0x20)
            loanPool := calldataload(0x40)
        }

        if (useLoan != 0) {
            address tokenBorrow;

            assembly {
                // the first tokenIn is the token we flashloan
                tokenBorrow := calldataload(0x80)
            }

            if (useLoan == 1) {
                // Balancer Flashloan
                IERC20[] memory tokens = new IERC20[](1);
                tokens[0] = IERC20(tokenBorrow);

                uint[] memory amounts = new uint[](1);
                amounts[0] = amountIn;

                IBalancerVault(loanPool).flashLoan(
                    IFlashLoanRecipient(address(this)),
                    tokens,
                    amounts,
                    msg.data
                );
            } else if (useLoan == 2) {
                // Uniswap V2 Flashswap
                IUniswapV2Pair pool = IUniswapV2Pair(loanPool);
                address token0 = pool.token0();
                if (tokenBorrow == token0) {
                    pool.swap(amountIn, 0, address(this), msg.data);
                } else {
                    pool.swap(0, amountIn, address(this), msg.data);
                }
            }
        } else {
            // perform swaps without flashloan
            _execute(msg.data);
        }
    }
}
