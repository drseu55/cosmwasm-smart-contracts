# cosmwasm-smart-contracts

Store contract example:

```sh
RES=$(wasmd tx wasm store ping_pong.wasm --from clean --node http://rpc.oysternet.cosmwasm.com:80 --chain-id oysternet-1 --gas auto --gas-prices 0.001usponge --gas-adjustment 1.2 --broadcast-mode block)
```
