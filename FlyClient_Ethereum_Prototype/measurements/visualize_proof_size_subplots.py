#!/bin/python
import matplotlib.pyplot as plt
import matplotlib
import pandas as pd
import numpy as np
import matplotlib     

proof_sizes = pd.read_csv('proof_size.csv')

df = pd.DataFrame(proof_sizes)


fig, axes = plt.subplots(2,1)

df.plot(ax=axes[0], x='block_number', y='blocks_queried', legend=False, color='r')
df.plot(ax=axes[0], x='block_number', y='L', legend=False, color='g')
df.plot(ax=axes[1], x='block_number', y='proof_size', legend=False)


axes[0].set_ylabel('Blocks queried', fontsize=20)
axes[0].set_xlabel('Block number', fontsize=20)
axes[0].set_xlim(0,7000000)
axes[0].set_ylim(0, 1100)
axes[0].tick_params(labelsize=15)
axes[0].yaxis.grid(True)
axes[0].xaxis.grid(True)

patches, labels = axes[0].get_legend_handles_labels()
axes[0].legend(patches, ["Queries", "L"], loc='upper left', fontsize=15)


axes[1].set_ylabel('Kilobyte', fontsize=20)
axes[1].set_xlabel('Block number', fontsize=20)
axes[1].set_xlim(0,7000000)
axes[1].set_ylim(0, 800)
axes[1].tick_params(labelsize=15)
axes[1].yaxis.grid(True)
axes[1].xaxis.grid(True)

patches, labels = axes[1].get_legend_handles_labels()
axes[1].legend(patches, ["Proof Size"], loc='upper left', fontsize=15)

plt.show()
