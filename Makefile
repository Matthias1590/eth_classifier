# train: src/train.py all_classified.csv
# 	unzip data.zip
# 	cat all_features_*.csv > all_features.csv
# 	python3 src/train.py

# bots.csv:
# 	@echo "TODO: Download from etherscan"
# 	@exit 1

# customers.csv:
# 	@echo "TODO: Download from random blocks and simple heuristics"
# 	@exit 1

# exchanges.csv:
# 	@echo "TODO: Download from dune query"
# 	@exit 1

# exchanges_with_trades.csv: exchanges.csv
# 	@echo "TODO: Use some script to fetch trades and volumes from etherscan"
# 	@exit 1

# all_classified.csv: classify_all.py exchanges_with_trades.csv bots.csv customers.csv
# 	python3 classify_all.py > all_classified.csv
