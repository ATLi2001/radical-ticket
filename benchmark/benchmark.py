import requests
import time
import pandas as pd

# base url
target = "http://localhost:8787"

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
  assert resp.status_code == 200

  # milliseconds
  return (end - start) * 1000


if __name__ == "__main__":
  n = 10
  trials = 10

  results = pd.DataFrame(columns=[f"ticket{i}_ms" for i in range(n)])

  for _ in range(trials):
    clear_kv()
    time.sleep(1)
    populate_tickets(n)
    time.sleep(1)
    trial_results = []
    for i in range(n):
      trial_results.append(reserve_ticket(i))
    
    results.loc[len(results)] = trial_results
  
  results.to_csv(f"simple_{n}tickets_{trials}trials.csv")
