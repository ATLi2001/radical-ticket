mod cache;

use cache::CacheKV;

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

type RWSet = Vec<String>;

#[derive(Serialize, Deserialize, Debug)]
pub struct RWSetResponse {
    status: u16,
    rw_set: RWSet,
}

// create new tickets 
// expected request body with number
async fn populate_tickets(mut req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    // extract request number
    let n = req.text().await?.parse::<u32>().unwrap();

    let cache = CacheKV::new().await;

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

        cache.put(&format!("ticket-{i}"), &val).await?;
    }
    
    // save in cache so we can know how much to clear later
    
    cache.put("count", &n).await?;

    Response::ok("")
}

// clear the entire cache of tickets
async fn clear_cache(_req:Request, _ctx: RouteContext<()>) -> Result<Response> {
    let cache = CacheKV::new().await;
    // get count of how many tickets total
    let n = cache.get::<u32>("count").await?.unwrap();

    for i in 0..n {
        cache.delete(&format!("ticket-{i}")).await?;
    }

    // reset count
    cache.put("count", &0).await?;

    Response::ok("Successfully cleared cache")
}

// return a specific ticket
async fn get_ticket(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(ticket_id) = ctx.param("id") {
        let id = ticket_id.parse::<u32>().expect("ticket id should be a number");
        let cache = CacheKV::new().await;
        match cache.get::<MyValue>(&format!("ticket-{id}")).await? {
            Some(val) => {
                Response::from_json(&val.value)
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

// multiply an input vector by a random normal matrix, returning an output vector
fn multiply_random_normal(input_vec: Vec<f32>, output_dim: usize, scale: f32) -> Vec<f32> {
    let normal = Normal::new(0.0, scale).unwrap();

    let mut normal_matrix = vec![vec![0f32; input_vec.len()]; output_dim];
    for i in 0..normal_matrix.len() {
        for j in 0..normal_matrix[i].len() {
            normal_matrix[i][j] = normal.sample(&mut rand::thread_rng());
        }
    }

    // output = (normal_matrix)(input_vec)
    let mut output = vec![0f32; output_dim];
    for i in 0..output.len() {
        for j in 0..input_vec.len() {
            output[i] += normal_matrix[i][j] * input_vec[j];
        }
    }

    output
}

// compute relu of input vector
fn relu(input_vec: Vec<f32>) -> Vec<f32> {
    let mut output = vec![0f32; input_vec.len()];
    for i in 0..output.len() {
        if input_vec[i] > 0.0 {
            output[i] = input_vec[i];
        }
    }

    output
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

    let model_depth = 50;
    for i in 0..model_depth {
        feature_vec = multiply_random_normal(feature_vec, 128, (i+1) as f32);
        feature_vec = relu(feature_vec);
    }

    true
}

// reserve a ticket
// expect request as a json with form of Ticket
async fn reserve_ticket(mut req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let ticket = req.json::<Ticket>().await?;
    let ticket_id = ticket.id;

    let cache = CacheKV::new().await;

    // get old val to compute new version number
    let resp = cache.get::<MyValue>(&format!("ticket-{ticket_id}")).await?;
    if resp.is_none() {
        return Response::error("not found", 500);
    }
    
    let old_val = resp.unwrap();
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
    cache.put(&format!("ticket-{ticket_id}"), &new_val).await?;

    Response::ok("Success")
}

// extract the read write set from the request
async fn get_rw_set(mut req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let ticket = req.json::<Ticket>().await?;
    let ticket_id = ticket.id;

    let rw_set: RWSet = vec![format!("ticket-{ticket_id}")];
    
    Response::from_json(&RWSetResponse {
        status: 200,
        rw_set,
    })
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new();
    router
        .get("/hello", |_, _| Response::ok("Hello, World!"))
        .get_async("/get_ticket/:id", get_ticket)
        .post_async("/populate_tickets", populate_tickets)
        .post_async("/reserve", reserve_ticket)
        .get_async("/rw_set", get_rw_set)
        .post_async("/clear_cache", clear_cache)
        .run(req, env)
        .await
}
