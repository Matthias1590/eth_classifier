import csv

# 0 = cold exchange
# 1 = hot exchange
# 2 = bot
# 3 = customer

with open('exchanges_with_trades.csv', 'r') as f:
    reader = csv.reader(f)
    for row in reader:
        addr, trades, volume = row
        trades = int(trades)
        volume = int(volume)
        if trades > 30 and volume > 10000000:
            print(f"{addr},1")
        else:
            print(f"{addr},0")

with open('bots.csv', 'r') as f:
    reader = csv.reader(f)
    for row in reader:
        addr = row[0]
        print(f"{addr},2")

with open('customers.csv', 'r') as f:
    reader = csv.reader(f)
    for row in reader:
        addr = row[0]
        print(f"{addr},3")
