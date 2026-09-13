use std::convert::TryInto;

use super::models::{DepthLevel, FullPacket, TickerPacket};

pub fn parse_ticker(data: &[u8]) -> Result<TickerPacket, String> {
    if data.len() < 16 {
        return Err(format!("ticker packet too short: {} bytes", data.len()));
    }

    let response_code = data[0];

    let message_length = u16::from_le_bytes(
        data[1..3]
            .try_into()
            .map_err(|_| "invalid message length")?,
    );

    let exchange_segment = data[3];

    let security_id = i32::from_le_bytes(data[4..8].try_into().map_err(|_| "invalid security id")?);

    let ltp = f32::from_le_bytes(data[8..12].try_into().map_err(|_| "invalid LTP")?);

    let last_trade_time = i32::from_le_bytes(data[12..16].try_into().map_err(|_| "invalid LTT")?);

    Ok(TickerPacket {
        response_code,
        message_length,
        exchange_segment,
        security_id,
        ltp,
        last_trade_time,
    })
}

pub fn parse_full(data: &[u8]) -> Result<FullPacket, String> {
    const FULL_PACKET_SIZE: usize = 162;

    if data.len() < FULL_PACKET_SIZE {
        return Err(format!(
            "full packet too short: expected {} bytes, got {}",
            FULL_PACKET_SIZE,
            data.len()
        ));
    }

    // ---------------------------------------------------------
    // Header: bytes 0..8
    // ---------------------------------------------------------

    let response_code = data[0];

    if response_code != 8 {
        return Err(format!(
            "expected FULL response code 8, got {}",
            response_code
        ));
    }

    let message_length = u16::from_le_bytes(
        data[1..3]
            .try_into()
            .map_err(|_| "invalid message length")?,
    );

    let exchange_segment = data[3];

    let security_id = i32::from_le_bytes(data[4..8].try_into().map_err(|_| "invalid security id")?);

    // ---------------------------------------------------------
    // Quote data: bytes 8..62
    // ---------------------------------------------------------

    let ltp = f32::from_le_bytes(data[8..12].try_into().map_err(|_| "invalid LTP")?);

    let last_trade_qty = i16::from_le_bytes(
        data[12..14]
            .try_into()
            .map_err(|_| "invalid last trade quantity")?,
    );

    let last_trade_time = i32::from_le_bytes(
        data[14..18]
            .try_into()
            .map_err(|_| "invalid last trade time")?,
    );

    let avg_trade_price = f32::from_le_bytes(
        data[18..22]
            .try_into()
            .map_err(|_| "invalid average trade price")?,
    );

    let volume = i32::from_le_bytes(data[22..26].try_into().map_err(|_| "invalid volume")?);

    let total_sell_qty = i32::from_le_bytes(
        data[26..30]
            .try_into()
            .map_err(|_| "invalid total sell quantity")?,
    );

    let total_buy_qty = i32::from_le_bytes(
        data[30..34]
            .try_into()
            .map_err(|_| "invalid total buy quantity")?,
    );

    let open_interest = i32::from_le_bytes(
        data[34..38]
            .try_into()
            .map_err(|_| "invalid open interest")?,
    );

    let highest_oi = i32::from_le_bytes(data[38..42].try_into().map_err(|_| "invalid highest OI")?);

    let lowest_oi = i32::from_le_bytes(data[42..46].try_into().map_err(|_| "invalid lowest OI")?);

    let day_open = f32::from_le_bytes(data[46..50].try_into().map_err(|_| "invalid day open")?);

    let day_close = f32::from_le_bytes(data[50..54].try_into().map_err(|_| "invalid day close")?);

    let day_high = f32::from_le_bytes(data[54..58].try_into().map_err(|_| "invalid day high")?);

    let day_low = f32::from_le_bytes(data[58..62].try_into().map_err(|_| "invalid day low")?);

    // ---------------------------------------------------------
    // Market depth: bytes 62..162
    //
    // 5 levels × 20 bytes
    // ---------------------------------------------------------

    let mut depth = Vec::with_capacity(5);

    for level in 0..5 {
        let offset = 62 + level * 20;

        let bid_qty = i32::from_le_bytes(
            data[offset..offset + 4]
                .try_into()
                .map_err(|_| "invalid bid quantity")?,
        );

        let ask_qty = i32::from_le_bytes(
            data[offset + 4..offset + 8]
                .try_into()
                .map_err(|_| "invalid ask quantity")?,
        );

        let bid_orders = i16::from_le_bytes(
            data[offset + 8..offset + 10]
                .try_into()
                .map_err(|_| "invalid bid orders")?,
        );

        let ask_orders = i16::from_le_bytes(
            data[offset + 10..offset + 12]
                .try_into()
                .map_err(|_| "invalid ask orders")?,
        );

        let bid_price = f32::from_le_bytes(
            data[offset + 12..offset + 16]
                .try_into()
                .map_err(|_| "invalid bid price")?,
        );

        let ask_price = f32::from_le_bytes(
            data[offset + 16..offset + 20]
                .try_into()
                .map_err(|_| "invalid ask price")?,
        );

        depth.push(DepthLevel {
            bid_qty,
            ask_qty,
            bid_orders,
            ask_orders,
            bid_price,
            ask_price,
        });
    }

    Ok(FullPacket {
        response_code,
        message_length,
        exchange_segment,
        security_id,

        ltp,
        last_trade_qty,
        last_trade_time,
        avg_trade_price,
        volume,

        total_sell_qty,
        total_buy_qty,

        open_interest,
        highest_oi,
        lowest_oi,

        day_open,
        day_close,
        day_high,
        day_low,

        depth,
    })
}
