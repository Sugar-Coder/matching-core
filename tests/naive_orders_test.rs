use std::collections::HashMap;
use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, NaiveOrderBook};

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
    println!("\n================");
    let l2 = orderbook.get_l2_data(10);
    println!("Ask: depth={} volume={}", orderbook.get_ask_buckets_count(), orderbook.get_total_ask_volume());
    for (price, vol) in l2.ask_prices.iter().rev().zip(l2.ask_volumes.iter().rev()) {
        println!("  {} x {}", price, vol);
    }

    println!("---");

    println!("Bid：depth={} volume={}", orderbook.get_bid_buckets_count(), orderbook.get_total_bid_volume());
    for (price, vol) in l2.bid_prices.iter().rev().zip(l2.bid_volumes.iter().rev()) {
        println!("  {} x {}", price, vol);
    }
    println!("================\n");
}

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

#[test]
fn basic_order_book_test() {
    let spec = create_spec();

    let mut orderbook = NaiveOrderBook::new(spec);

    let mut ask_order = OrderCommand {
        uid: 1,                         // User ID
        order_id: 1,                    // Unique order identifier
        symbol: 1,                      // Must match spec.symbol_id
        price: 10000,                   // Price in quote currency
        size: 100,                      // Size in base currency
        action: OrderAction::Ask,       // Sell order
        order_type: OrderType::Gtc,     // Good-Till-Cancel
        reserve_price: 10000,           // Internal price tracking
        timestamp: 1000,                // Order timestamp
        ..Default::default()
    };

    orderbook.new_order(&mut ask_order); // after placed order, the OrderCommand is mutated in place

    let mut bid_order = OrderCommand {
        uid: 2,
        order_id: 1,
        symbol: 1,                      // Must match spec.symbol_id
        price: 10000,                   // Same as ask price
        size: 50,                       // Partial fill (100 available)
        action: OrderAction::Bid,       // Buy order
        order_type: OrderType::Ioc,     // Immediate-or-Cancel
        reserve_price: 10000,
        timestamp: 1001,
        ..Default::default()
    };

    let res = orderbook.new_order(&mut bid_order);
    if res != CommandResultCode::Success {
        println!("Order creation failed: {:?}", res);
        return;
    }

    println!("bid_order events");
    for event in &bid_order.matcher_events {
        println!("MatcherTradeEvent: \n{:?}", event);
    }

    println!("ask_order events:");
    for event in &ask_order.matcher_events {
        println!("MatcherTradeEvent: {:?}", event);
    }

    let mut bid_order2 = OrderCommand {
        uid: 2,
        order_id: 3,
        symbol: 1,
        price: 10000,
        size: 100,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 1002,
        ..Default::default()
    };

    orderbook.new_order(&mut bid_order2);
    println!("bid_order2 events");
    for event in &bid_order2.matcher_events {
        println!("MatcherTradeEvent: \n{:?}", event);
    }
}

#[test]
fn naive_order_book_test() {
    let spec = create_spec();
    let mut orderbook = NaiveOrderBook::new(spec);
    let mut bid_orders = HashMap::new();
    let mut ask_orders = HashMap::new();

    let bids = add_depth(&mut orderbook, 1, 1, 5, OrderAction::Bid, 500, 10, 20, 1000);
    bids.into_iter().for_each(|item| { bid_orders.insert(item.order_id, item); });
    let bids = add_depth(&mut orderbook, 2, 6, 5, OrderAction::Bid, 500, 10, 20, 1000);
    bids.into_iter().for_each(|item| { bid_orders.insert(item.order_id, item); });

    let asks = add_depth(&mut orderbook, 3, 11, 5, OrderAction::Ask, 510, 10, 20, 1000);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });
    let asks = add_depth(&mut orderbook, 4, 16, 5, OrderAction::Ask, 510, 10, 20, 1000);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });

    print_orderbook(&mut orderbook);


    let mut ask_order_one = OrderCommand {
        uid: 5,
        order_id: 21,
        symbol: 1,
        price: 490 * 80, // the minimum total transaction price I accept (for Ask)
        size: 80, // the size I want to fill
        action: OrderAction::Ask,
        order_type: OrderType::FokBudget,
        reserve_price: 0,
        timestamp: 1003,
        ..Default::default()
    };

    let res = orderbook.new_order(&mut ask_order_one);
    if res != CommandResultCode::Success {
        println!("new_order result: {:?}", res);
        return;
    }

    println!("ask_order_one events");
    for event in &ask_order_one.matcher_events {
        println!("{:?}", event);
        if event.matched_order_id == 0 {
            continue;
        }
        let matched_order = orderbook.get_order_by_id(event.matched_order_id).unwrap();
        println!("matched_order: {:?}", matched_order);
        let bid_order = bid_orders.get(&event.matched_order_id).unwrap();
        println!("\tbid_order: {:?}", bid_order);
    }

    print_orderbook(&mut orderbook);
}

#[test]
fn move_order_test() {
    let spec = create_spec();
    let mut orderbook = NaiveOrderBook::new(spec);
    let mut bid_orders = HashMap::new();
    let mut ask_orders = HashMap::new();

    let bids = add_depth(&mut orderbook, 1, 1, 5, OrderAction::Bid, 500, 10, 20, 1000);
    bids.into_iter().for_each(|item| { bid_orders.insert(item.order_id, item); });
    let bids = add_depth(&mut orderbook, 2, 6, 5, OrderAction::Bid, 500, 10, 20, 1001);
    bids.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });

    let asks = add_depth(&mut orderbook, 3, 11, 5, OrderAction::Ask, 510, 10, 20, 1000);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });
    let asks = add_depth(&mut orderbook, 4, 16, 5, OrderAction::Ask, 510, 10, 20, 1001);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });

    print_orderbook(&mut orderbook);

    // make a deal
    let mut oc1 = OrderCommand {
        uid: 5,
        order_id: 21,
        symbol: 1,
        price: 510,
        size: 50,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 550,
        timestamp: 1002,
        ..Default::default()
    };
    let res = orderbook.new_order(&mut oc1);
    if res != CommandResultCode::Success {
        println!("new_order result: {:?}", res);
        return;
    }
    for event in &oc1.matcher_events {
        println!("{:?}", event);
        if event.matched_order_id == 0 {
            continue;
        }
        let matched_order = orderbook.get_order_by_id(event.matched_order_id).unwrap();
        println!("matched_order: {:?}", matched_order);
        let bid_order = ask_orders.get(&event.matched_order_id).unwrap();
        println!("\task_order: {:?}", bid_order);
    }

    print_orderbook(&mut orderbook);

    println!("move_order id=12 price from 520 -> 500");
    // move order: make existing best ask order lower price
    let mut oc2 = OrderCommand {
        uid: 3,
        order_id: 12,
        symbol: 1,
        price: 500,
        action: OrderAction::Ask,
        timestamp: 1003,
        ..Default::default()
    };
    let res = orderbook.move_order(&mut oc2);
    if res != CommandResultCode::Success {
        println!("move_order result: {:?}", res);
    }

    // check result
    for event in &oc2.matcher_events {
        println!("{:?}", event);
        if event.matched_order_id == 0 {
            continue;
        }
        let matched_order = orderbook.get_order_by_id(event.matched_order_id).unwrap();
        println!("matched_order: {:?}", matched_order);
        let bid_order = bid_orders.get(&event.matched_order_id).unwrap();
        println!("\tbid_order: {:?}", bid_order);
    }

    print_orderbook(&mut orderbook);
}

#[test]
fn cancel_order_test() {
    let spec = create_spec();
    let mut orderbook = NaiveOrderBook::new(spec);
    let mut bid_orders = HashMap::new();
    let mut ask_orders = HashMap::new();

    let bids = add_depth(&mut orderbook, 1, 1, 5, OrderAction::Bid, 500, 10, 20, 1000);
    bids.into_iter().for_each(|item| { bid_orders.insert(item.order_id, item); });
    let bids = add_depth(&mut orderbook, 2, 6, 5, OrderAction::Bid, 500, 10, 20, 1001);
    bids.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });

    let asks = add_depth(&mut orderbook, 3, 11, 5, OrderAction::Ask, 510, 10, 20, 1000);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });
    let asks = add_depth(&mut orderbook, 4, 16, 5, OrderAction::Ask, 510, 10, 20, 1001);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });

    print_orderbook(&mut orderbook);

    let mut oc1 = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        ..Default::default()
    };
    let res = orderbook.cancel_order(&mut oc1);
    if res != CommandResultCode::Success {
        println!("new_order result: {:?}", res);
    }

    // check events
    for event in &oc1.matcher_events {
        println!("{:?}", event);
    }
    print_orderbook(&mut orderbook);
}

#[test]
fn reduce_order_test() {
    let spec = create_spec();
    let mut orderbook = NaiveOrderBook::new(spec);
    let mut bid_orders = HashMap::new();
    let mut ask_orders = HashMap::new();

    let bids = add_depth(&mut orderbook, 1, 1, 5, OrderAction::Bid, 500, 10, 20, 1000);
    bids.into_iter().for_each(|item| { bid_orders.insert(item.order_id, item); });

    let asks = add_depth(&mut orderbook, 2, 6, 5, OrderAction::Ask, 510, 10, 20, 1000);
    asks.into_iter().for_each(|item| { ask_orders.insert(item.order_id, item); });

    print_orderbook(&mut orderbook);

    let mut oc1 = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        size: 10, // reduce by size
        ..Default::default()
    };

    let res = orderbook.reduce_order(&mut oc1);
    if res != CommandResultCode::Success {
        println!("reduce_order result: {:?}", res);
    }

    for event in &oc1.matcher_events {
        println!("{:?}", event);
    }

    print_orderbook(&mut orderbook);
}