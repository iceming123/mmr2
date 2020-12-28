#!/bin/python
import matplotlib.pyplot as plt
import matplotlib
import pandas as pd
import numpy as np
import matplotlib     

proof_sizes = pd.read_csv('measurementsTradeoff.csv')

df = pd.DataFrame(proof_sizes)


fig, axes = plt.subplots(2, 1)

df.plot(ax=axes[0], x='l', y='complete_validation_time', legend=False, color='r')
df.plot(ax=axes[1], x='l', y='complete_proof_size', legend=False)

axes[0].set_ylabel('Seconds', fontsize=20)
axes[0].set_xlabel('L', fontsize=20)
axes[0].set_xlim(100,1000)
axes[0].set_ylim(0, 700)
axes[0].tick_params(labelsize=15)
axes[0].yaxis.grid(True)
axes[0].xaxis.grid(True)

patches, labels = axes[0].get_legend_handles_labels()
axes[0].legend(patches, ["Complete Validation Time"], loc='upper left', fontsize=15)

axes[1].set_ylabel('Kilobyte', fontsize=20)
axes[1].set_xlabel('L', fontsize=20)
axes[1].set_xlim(100,1000)
axes[1].set_ylim(0, 1300)
axes[1].tick_params(labelsize=15)
axes[1].yaxis.grid(True)
axes[1].xaxis.grid(True)

patches, labels = axes[1].get_legend_handles_labels()
axes[1].legend(patches, ["Proof Size"], loc='upper left', fontsize=15)


plt.show()
