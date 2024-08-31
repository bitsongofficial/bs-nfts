#!/bin/bash

# Define the contract address and any other required variables
CURVE_ADDRESS="bitsong18y8luta52kdxlm56d4t4xvyfqlgku0emmu964gtqhpl7xzq698fq7t6l3d"
QUERYFLAG="--node https://rpc.bwasmnet-1.bitsong.network:443 --output json"

# The output CSV file
OUTPUT_FILE="query_results.csv"

# Write the CSV header
echo "id,base_price,royalties,referral,protocol_fee,total_price" > "$OUTPUT_FILE"

# Loop 1000 times
for i in {1..1000}
do
    # Modify QUERY_PRICE with the current loop value for 'amount'
    QUERY_PRICE="{\"buy_price\":{\"amount\":\"$i\"}}"

    # Perform the query
    RESPONSE=$(bitsongd query wasm contract-state smart $CURVE_ADDRESS "$QUERY_PRICE" $QUERYFLAG)

    # Extract data from the JSON response
    BASE_PRICE=$(echo $RESPONSE | jq -r '.data.base_price')
    ROYALTIES=$(echo $RESPONSE | jq -r '.data.royalties')
    REFERRAL=$(echo $RESPONSE | jq -r '.data.referral')
    PROTOCOL_FEE=$(echo $RESPONSE | jq -r '.data.protocol_fee')
    TOTAL_PRICE=$(echo $RESPONSE | jq -r '.data.total_price')

    # Append the data to the CSV file
    echo "$i,$BASE_PRICE,$ROYALTIES,$REFERRAL,$PROTOCOL_FEE,$TOTAL_PRICE" >> "$OUTPUT_FILE"
done
