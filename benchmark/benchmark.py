import requests
import time
import pandas as pd
import argparse


# populate the kv with n tickets
def populate_tickets(n):
  url = target + "/populate_tickets"
  resp = requests.post(url, data=f"{n}")
  assert resp.status_code == 200

# clear the kv
def clear_kv():
  url = target + "/clear_kv"
  resp = requests.post(url)
  assert resp.status_code == 200
  print(resp.text)

# list all available tickets
def avail_tickets():
  resp = requests.get(target)
  if resp.status_code == 200: 
    print(resp.content)
  else:
    print("avail_tickets error", resp.status_code)

# get ticket i
def get_ticket(i):
  url = target + f"/get_ticket/{i}"
  resp = requests.get(url)
  assert resp.status_code == 200
  return resp.text

# reserve ticket i and return the time it took in ms
def reserve_ticket(i):
  ticket_data = {
    "id": i,
    "taken": True,
    "res_email": "test@test.com",
    "res_name": "Test Name", 
    "res_card": "xxxx1234", 
  }

  url = target + "/reserve"
  start = time.perf_counter()
  resp = requests.post(url, json=ticket_data)
  end = time.perf_counter()
  if resp.status_code != 200:
    print(f"reserve_ticket({i})", resp)

  # milliseconds
  return (end - start) * 1000


if __name__ == "__main__":
  # Parse in command-line arguments.
  parser = argparse.ArgumentParser(
      prog="ticket-benchmark",
      description="Creates tickets and measures latency of reserving a ticket",
  )
  parser.add_argument("-d", "--dev", action="store_true", help="use the local dev server rather than the Cloudflare deployment")
  args = parser.parse_args()

  # Set target depending on dev vs. prod.
  if args.dev:
      target = "http://localhost:8787"
      env_name = "local"
  else:
      target = "http://ticket-bench.sns-radical.workers.dev"
      env_name = "edge"

  n = 10
  trials = 10

  results = pd.DataFrame(columns=[f"ticket{i}_ms" for i in range(n)])

  # use different tickets on each trial
  clear_kv()
  time.sleep(1)
  populate_tickets(n * trials)
  time.sleep(1)

  for t in range(trials):
    avail_tickets()
    trial_results = []
    for i in range(n):
      print(get_ticket(n*t + i))
      trial_results.append(reserve_ticket(n*t + i))
    
    results.loc[len(results)] = trial_results
  
  results.to_csv(f"simple_{env_name}_{n}tickets_{trials}trials.csv")
