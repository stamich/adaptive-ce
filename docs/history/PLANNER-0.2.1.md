# Planner V2.1

ACE 0.2.1 addresses the 0.2 benchmark's low candidate recall by separating candidate origin from final cost ranking.

`Mandatory` candidates protect stable baselines. `Likely` candidates follow strong analyzer evidence. `Exploratory` candidates widen the search when the block evidence is uncertain or the DENSE profile explicitly favors size exploration.

Cost Model V2.1 includes encoded metadata bytes and static codec/entropy work coefficients. Wall-clock timing remains benchmark-only and is never an input to runtime plan choice.
