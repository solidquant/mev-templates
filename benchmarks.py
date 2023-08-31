import os
import datetime
import pandas as pd

from pathlib import Path

_DIR = Path(os.path.dirname(os.path.abspath(__file__)))

def df_fmt(df: pd.DataFrame, name: str) -> pd.DataFrame:
    df.columns = ['tx_hash', name]
    df['tx_hash'] = df['tx_hash'].apply(lambda x: x.lower())
    # df[name] = df[name].apply(lambda x: datetime.datetime.fromtimestamp(x / 1000000).strftime('%H:%M:%S.%f'))
    return df


if __name__ == '__main__':
    js = df_fmt(pd.read_csv(_DIR / 'javascript/benches/.benchmark.csv', header=None), 'js')
    py = df_fmt(pd.read_csv(_DIR / 'python/benches/.benchmark.csv', header=None), 'py')
    rs = df_fmt(pd.read_csv(_DIR / 'rust/benches/.benchmark.csv', header=None), 'rs')
    
    bench = js.merge(py, on='tx_hash').merge(rs, on='tx_hash')
    bench['py - rs'] = bench['py'] - bench['rs']
    bench['js - rs'] = bench['js'] - bench['rs']
    bench['py - js'] = bench['py'] - bench['js']
    bench = bench.drop_duplicates(['tx_hash'], keep='last')
    
    bench.to_csv(_DIR / '.benchmark.csv', index=None)