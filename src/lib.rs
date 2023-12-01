use std::vec;

use worker::*;
use serde::{Serialize, Deserialize};
use regex::Regex;
use rand_distr::{Normal, Distribution};

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

// helper function for cache read
async fn cache_read(id: u32) -> Option<MyValue> {
    let cache = Cache::default();
    let cache_uri = format!("http://radicalcache/key/ticket-{id}");
    let resp = cache.get(cache_uri, false).await.unwrap();
    match resp {
        Some(r) => {
            // manually add application/json headers to get json() to work
            let mut h = Headers::new();
            h.append("Content-Type", "application/json").unwrap();
            let mut r_json = r.with_headers(h);
            Some(r_json.json::<MyValue>().await.unwrap())
        }, 
        None => None
    }
}

// helper function for cache write
async fn cache_write(id: u32, val: &MyValue) {
    let cache = Cache::default();
    let cache_uri = format!("http://radicalcache/key/ticket-{id}");
    let mut cache_headers = Headers::new();
    cache_headers.append("Cache-Control", "max-age=1000").unwrap();
    cache_headers.append("Cache-Control", "public").unwrap();
    let cache_resp = Response::from_json::<MyValue>(val).unwrap().with_headers(cache_headers);

    cache.put(cache_uri, cache_resp).await.unwrap()
}

// create new tickets 
// expected request body with number
async fn populate_tickets(mut req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    // extract request number
    let n = req.text().await?.parse::<u32>().unwrap();

    // create n tickets 
    for i in 0..n {
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

        cache_write(i, &val).await;
    }

    Response::ok("")
}

// return a specific ticket
async fn get_ticket(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(ticket_id) = ctx.param("id") {
        let id = ticket_id.parse::<u32>().expect("ticket id should be a number");
        let val = cache_read(id).await;
        match val {
            Some(v) => {
                Response::from_json(&v.value)
            },
            None => {
                Response::error("not found", 500)
            }
        }
    } 
    else {
        Response::error("Bad request", 400)
    }
}

// check if ticket reservation passes anti fraud test
// true means reservation is ok, false means not
fn anti_fraud(ticket: &Ticket) -> bool {
    // valid email must have some valid characters before @, some after, a dot, then some more
    const EMAIL_REGEX: &str = r"(^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$)";
    let re = Regex::new(EMAIL_REGEX).unwrap();

    if !re.is_match(&ticket.res_email.clone().unwrap()) {
        return false;
    }

    // check "ml" model
    // create a feature vector from the name, email, and card
    let feature_str = [
        ticket.res_name.clone().unwrap().as_bytes(), 
        ticket.res_email.clone().unwrap().as_bytes(), 
        ticket.res_card.clone().unwrap().as_bytes(),
    ].concat();
    // feature vector is normalized
    let mut feature_vec = vec![0f32; feature_str.len()];
    let mut feature_norm = 0.0;
    for i in 0..feature_str.len() {
        feature_norm += (feature_str[i] as f32).powi(2);
    }
    for i in 0..feature_vec.len() {
        feature_vec[i] = (feature_str[i] as f32) / (feature_norm.sqrt());
    }

    let normal = Normal::new(0.0, 1.0).unwrap();
    // create OUTPUT_LENGTH x feature_vec random matrix
    const OUTPUT_LENGTH: usize = 128;
    let mut normal_matrix = vec![vec![0f32; feature_vec.len()]; OUTPUT_LENGTH];
    for i in 0..normal_matrix.len() {
        for j in 0..normal_matrix[i].len() {
            normal_matrix[i][j] = normal.sample(&mut rand::thread_rng());
        }
    }
    // output = (normal_matrix)(feature_vec)
    let mut output = vec![0f32; OUTPUT_LENGTH];
    for i in 0..output.len() {
        for j in 0..feature_vec.len() {
            output[i] += normal_matrix[i][j] * feature_vec[j];
        }
    }

    true
}

// reserve a ticket
// expect request as a json with form of Ticket
async fn reserve_ticket(mut req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let ticket = req.json::<Ticket>().await?;
    let ticket_id = ticket.id;

    // get old val to compute new version number
    let val = cache_read(ticket_id).await;
    if val.is_none() {
        return Response::error("not found", 500);
    }
    
    let old_val = val.unwrap();
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

    // put back into cache
    cache_write(ticket_id, &new_val).await;

    Response::ok("Success")
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new();
    router
        .get("/hello", |_, _| Response::ok("Hello, World!"))
        .get_async("/get_ticket/:id", get_ticket)
        .post_async("/populate_tickets", populate_tickets)
        .post_async("/reserve", reserve_ticket)
        .run(req, env)
        .await
}
