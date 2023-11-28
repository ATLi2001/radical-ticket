use worker::*;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Ticket {
    pub id: u32,
    pub taken: bool,
    // reservation details, only filled out if taken=true
    pub res_email: Option<String>,
    pub res_name: Option<String>,
    pub res_card: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct MyValue {
    pub version: u32,
    pub value: Ticket,
}

// create new tickets 
// expected request body with number
async fn populate_tickets(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let kv = ctx.kv("RADICAL_TICKET_KV")?;

    // extract request number
    let n = req.text().await?.parse::<u32>().unwrap();

    // create n tickets 
    for i in 0..n {
        let key = format!("ticket-{i}");
        let ticket = Ticket { 
            id: i,
            taken: false,
            res_email: None,
            res_name: None,
            res_card: None,
        };
        let val = MyValue {
            version: 0,
            value: ticket,
        };

        kv.put(&key, val)?.execute().await?;
    }

    Response::ok("")
}

async fn clear_kv(_req:Request, ctx: RouteContext<()>) -> Result<Response> {
    let kv = ctx.kv("RADICAL_TICKET_KV")?;
    let keys = kv.list().execute().await?.keys;
    for key in keys {
        kv.delete(&key.name).await?;
    }

    Response::ok("Successfully cleared kv")
}

// return all available tickets (taken=false)
async fn get_index(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let kv = ctx.kv("RADICAL_TICKET_KV")?;
    let keys = kv.list().execute().await?.keys;

    let mut avail_tickets = "".to_owned();
    for key in keys {
        let val: MyValue = kv.get(&key.name).json().await?.unwrap();
        if !val.value.taken {
            avail_tickets.push_str(&val.value.id.to_string());
            avail_tickets.push_str("\n");
        }
    }

    Response::ok(avail_tickets)
}

// return a specific ticket
async fn get_ticket(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(ticket_id) = ctx.param("id") {
        let kv = ctx.kv("RADICAL_TICKET_KV")?;
        let key = format!("ticket-{ticket_id}");
        let val: MyValue = kv.get(&key).json().await?.unwrap();
        Response::from_json(&val.value)
    } 
    else {
        Response::error("Bad request", 400)
    }
}

// check if ticket reservation passes anti fraud test
// true means reservation is ok, false means not
fn anti_fraud(_ticket: &Ticket) -> bool {
    true
}

// reserve a ticket
// expect request as a json with form of Ticket
async fn reserve_ticket(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let kv = ctx.kv("RADICAL_TICKET_KV")?;

    let ticket = req.json::<Ticket>().await?;
    let ticket_id = ticket.id;

    // get old val to compute new version number
    let key = format!("ticket-{ticket_id}");
    let old_val: MyValue = kv.get(&key).json().await?.unwrap();
    let new_version = old_val.version + 1;
    // check that the ticket is not already taken
    if old_val.value.taken {
        return Response::error("ticket already reserved", 500);
    }
    
    // create new ticket that is taken while checking reservation details are given
    let new_ticket = Ticket {
        id: ticket_id,
        taken: true,
        res_email: Some(ticket.res_email.expect("no reservation email")),
        res_name: Some(ticket.res_name.expect("no reservation name")),
        res_card: Some(ticket.res_card.expect("no reservation card")),
    };

    // call anti fraud detection
    if !anti_fraud(&new_ticket) {
        return Response::error("fraudulent reservation detected", 500);
    }

    let new_val = MyValue {
        version: new_version,
        value: new_ticket,
    };
    // put back into kv
    kv.put(&key, new_val)?.execute().await?;

    Response::ok("Success")
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new();
    router
        .get("/hello", |_, _| Response::ok("Hello, World!"))
        .get_async("/", get_index)
        .get_async("/get_ticket/:id", get_ticket)
        .post_async("/populate_tickets", populate_tickets)
        .post_async("/reserve", reserve_ticket)
        .post_async("/clear_kv", clear_kv)
        .run(req, env)
        .await
}
