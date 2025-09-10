import pandas as pd
from sklearn.model_selection import train_test_split
from sklearn.ensemble import RandomForestClassifier
from sklearn.metrics import classification_report

df = pd.read_csv("all_features.csv", header=None)
classes = pd.read_csv("all_classified.csv", header=None)

def get_label(address: str) -> int:
    row = classes[classes[0] == address]
    assert not row.empty
    return int(row.iloc[0, 1])  # type: ignore

# replace address column with label
df.iloc[:, 0] = df.iloc[:, 0].apply(get_label)

X = df.iloc[:, 1:]
y = df.iloc[:, 0].astype(int)

X_train, X_test, y_train, y_test = train_test_split(
    X, y, test_size=0.1, random_state=42, stratify=y
)

model = RandomForestClassifier(n_estimators=200, random_state=42)
model.fit(X_train, y_train)

y_pred = model.predict(X_test)
print(classification_report(y_test, y_pred))

import joblib
joblib.dump(model, "rf_model.joblib")
