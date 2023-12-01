import requests
import time
import pandas as pd
import argparse


class TicketBenchmark:
  def __init__(self, target: str) -> None:
    # base target url
    self.target = target
    # keep request session alive
    self.session = requests.Session()
  
  # populate the kv with n tickets
  def populate_tickets(self, n) -> None:
    url = self.target + "/populate_tickets"
    resp = self.session.post(url, data=f"{n}")
    assert resp.status_code == 200

  # clear the kv
  def clear_kv(self) -> None:
    url = target + "/clear_kv"
    resp = self.session.post(url)
    assert resp.status_code == 200
    print(resp.text)

  # list all available tickets
  def avail_tickets(self) -> None:
    resp = self.session.get(target)
    if resp.status_code == 200: 
      print(resp.content)
    else:
      print("avail_tickets error", resp.status_code)

  # get ticket i
  def get_ticket(self, i: int) -> str:
    url = target + f"/get_ticket/{i}"
    resp = self.session.get(url)
    assert resp.status_code == 200
    return resp.text

  # reserve ticket i and return the time it took in ms
  def reserve_ticket(self, i: int) -> float:
    ticket_data = {
      "id": i,
      "taken": True,
      "res_email": "test@test.com",
      "res_name": "Test Name", 
      "res_card": "xxxx1234", 
    }

    url = target + "/reserve"
    start = time.perf_counter()
    resp = self.session.post(url, json=ticket_data)
    end = time.perf_counter()
    if resp.status_code != 200:
      print(f"ERROR: reserve_ticket({i})", resp)

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
      target = "https://ticket-bench.radical-serverless.com"
      env_name = "edge"

  n = 10
  trials = 10

  results = pd.DataFrame(columns=[f"ticket{i}_ms" for i in range(n)])

  ticket_bench = TicketBenchmark(target)


  for t in range(trials):
    ticket_bench.populate_tickets(n)
    time.sleep(1)

    trial_results = []
    for i in range(n):
      print(ticket_bench.get_ticket(i))
      trial_results.append(ticket_bench.reserve_ticket(i))
    
    results.loc[len(results)] = trial_results
  
  results.to_csv(f"simple_{env_name}_{n}tickets_{trials}trials.csv")
