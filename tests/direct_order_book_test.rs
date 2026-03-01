use std::collections::HashMap;
use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, DirectOrderBook};


fn create_spec() -> CoreSymbolSpecification {
    CoreSymbolSpecification {
        symbol_id: 1,                                      // Unique identifier
        symbol_type: SymbolType::CurrencyExchangePair,     // Spot trading
        base_currency: 0,                                  // BTC
        quote_currency: 1,                                 // USDT
        base_scale_k: 1,                                   // Price precision
        quote_scale_k: 1,                                  // Size precision
        taker_fee: 10,                                     // 0.1% (basis points)
        maker_fee: 5,                                      // 0.05%
        margin_buy: 0,                                     // No margin requirement
        margin_sell: 0,
    }
}

fn add_depth(order_book: &mut impl OrderBook,
             uid: UserId,
             order_start_id: OrderId,
             order_cmd_count: Size,
             action: OrderAction,
             best_price: Price,
             step_price: Price,
             size: Size,
             timestamp: i64) -> Vec<OrderCommand> {
    let mut orders = Vec::new();
    let mut end_price = 0;
    let mut begin_price = 0;
    if action == OrderAction::Bid {
        begin_price = best_price - (order_cmd_count - 1) * step_price;
        end_price = best_price + step_price;
    } else if action == OrderAction::Ask {
        begin_price = best_price;
        end_price = best_price + order_cmd_count * step_price;
    }

    let mut oid = order_start_id;
    for (i, price) in (begin_price..end_price).step_by(10).enumerate() {
        orders.push(OrderCommand {
            uid,
            order_id: oid,
            symbol: 1,
            price,
            size,
            action,
            order_type: OrderType::Gtc,
            reserve_price: price,
            timestamp,
            ..Default::default()
        });
        oid += 1;
        let res = order_book.new_order(&mut orders[i]);
        if res != CommandResultCode::Success {
            println!("Order creation failed: {:?}", res);
        }
    }

    orders
}

fn print_orderbook(orderbook: &mut impl OrderBook) {
    // 价格从高到低
    println!("\n================");
    let l2 = orderbook.get_l2_data(10);
    println!("Ask: depth={} volume={}", orderbook.get_ask_buckets_count(), orderbook.get_total_ask_volume());
    for (price, vol) in l2.ask_prices.iter().rev().zip(l2.ask_volumes.iter().rev()) {
        println!("  {} x {}", price, vol);
    }

    println!("---");

    // the impl in direct order book make the bid price already reversed
    println!("Bid：depth={} volume={}", orderbook.get_bid_buckets_count(), orderbook.get_total_bid_volume());
    for (price, vol) in l2.bid_prices.iter().zip(l2.bid_volumes.iter()) {
        println!("  {} x {}", price, vol);
    }
    println!("================\n");
}

#[test]
fn direct_order_book_test() {
    let spec = create_spec();
    let mut orderbook = DirectOrderBook::new(spec);
    let mut bid_orders = HashMap::new();
    let mut ask_orders = HashMap::new();

    let bids = add_depth(&mut orderbook, 1, 1, 5, OrderAction::Bid, 500, 10, 20, 1000);
    bids.into_iter().for_each(|item| { bid_orders.insert(item.order_id, item); });
    let bids = add_depth(&mut orderbook, 1, 6, 5, OrderAction::Bid, 500, 10, 20, 1000);
    bids.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });

    let asks = add_depth(&mut orderbook, 2, 11, 5, OrderAction::Ask, 510, 10, 20, 1000);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });
    let asks = add_depth(&mut orderbook, 2, 16, 5, OrderAction::Ask, 510, 10, 20, 1000);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });


    print_orderbook(&mut orderbook);


    // let mut test_order = OrderCommand {
    //     uid: 3,
    //     order_id: 21,
    //     symbol: 1,
    //     price: 510 * 80, // for bid order, the maximum price I accept for this size transaction
    //     size: 80,
    //     action: OrderAction::Bid,
    //     order_type: OrderType::FokBudget,
    //     reserve_price: 0,
    //     timestamp: 1003,
    //     ..Default::default()
    // };

    let mut test_order = OrderCommand {
        uid: 3,
        order_id: 21,
        symbol: 1,
        price: 500 * 40, // for ask order, the minimum price I accept for this size transaction
        size: 40,
        action: OrderAction::Ask,
        order_type: OrderType::FokBudget,
        reserve_price: 0,
        timestamp: 1003,
        ..Default::default()
    };

    // let mut test_order = OrderCommand {
    //         uid: 3,
    //         order_id: 21,
    //         symbol: 1,
    //         price: 515, // for ask order, the minimum price I accept for this size transaction
    //         size: 40,
    //         action: OrderAction::Ask,
    //         order_type: OrderType::Gtc,
    //         reserve_price: 0,
    //         timestamp: 1003,
    //         ..Default::default()
    //     };

    let res = orderbook.new_order(&mut test_order);
    if res != CommandResultCode::Success {
        println!("new_order result: {:?}", res);
        return;
    }

    println!("test_order events");
    for event in &test_order.matcher_events {
        println!("{:?}", event);
    }

    print_orderbook(&mut orderbook);
}