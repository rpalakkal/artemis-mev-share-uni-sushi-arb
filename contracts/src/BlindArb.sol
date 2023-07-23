// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Owned} from "solmate/auth/Owned.sol";

interface IWETH {
    function deposit() external payable;
    function withdraw(uint256) external;
    function balanceOf(address) external view returns (uint256);
    function transfer(address, uint256) external returns (bool);
}

interface IUniswapV2Pair {
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data) external;
}

contract BlindArb is Owned {
    IWETH internal constant WETH = IWETH(0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2);

    constructor() Owned(msg.sender) {}

    function executeArb(
        address first,
        address second,
        uint256 amountIn,
        uint256 percentageToPayToCoinbase
    ) external onlyOwner {
        uint256 balanceBefore = WETH.balanceOf(address(this));
       
        IUniswapV2Pair firstPair = IUniswapV2Pair(first);
        (uint256 firstReserve0, uint256 firstReserve1,) = firstPair.getReserves();
        uint256 firstAmountOut = getAmountOut(amountIn, firstReserve1, firstReserve0);
        firstPair.swap(firstAmountOut, 0, address(this), "");

        IUniswapV2Pair secondPair = IUniswapV2Pair(second);
        (uint256 secondReserve0, uint256 secondReserve1,) = secondPair.getReserves();
        uint256 secondAmountOut = getAmountOut(firstAmountOut, secondReserve0, secondReserve1);
        secondPair.swap(0, secondAmountOut, address(this), "");

        uint256 balanceAfter = WETH.balanceOf(address(this));
        uint profit = balanceAfter - balanceBefore;
        uint profitToCoinbase = profit *  percentageToPayToCoinbase / 100;
        WETH.withdraw(profitToCoinbase);
        block.coinbase.transfer(profitToCoinbase);
        require(balanceAfter - profitToCoinbase > balanceBefore, "arb failed");

    }

    function getAmountOut(uint256 amountIn, uint256 reserveIn, uint256 reserveOut)
        internal
        pure
        returns (uint256 amountOut)
    {
        uint256 amountInWithFee = amountIn * 997;
        uint256 numerator = amountInWithFee * reserveOut;
        uint256 denominator = reserveIn * 1000 + amountInWithFee;
        amountOut = numerator / denominator;
    }

    function withdrawWETHToOwner() external onlyOwner {
        uint256 balance = WETH.balanceOf(address(this));
        WETH.transfer(msg.sender, balance);
    }

    function withdrawETHToOwner() external onlyOwner {
        uint256 balance = address(this).balance;
        payable(msg.sender).transfer(balance);
    }

    receive() external payable {}
}
