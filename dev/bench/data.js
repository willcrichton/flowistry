window.BENCHMARK_DATA = {
  "lastUpdate": 1653449110232,
  "repoUrl": "https://github.com/willcrichton/flowistry",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "wcrichto@cs.stanford.edu",
            "name": "Will Crichton",
            "username": "willcrichton"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ad9b274d2581010f09a1831498d371269d03367a",
          "message": "Merge pull request #51 from connorff/bench-deployment-fix\n\nFix bench results not saving in `gh-pages` branch",
          "timestamp": "2022-05-24T20:09:51-07:00",
          "tree_id": "ecf3c2149a7af5e3c4fda938ffce84b186383e14",
          "url": "https://github.com/willcrichton/flowistry/commit/ad9b274d2581010f09a1831498d371269d03367a"
        },
        "date": 1653449030827,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 188483,
            "range": "± 253",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 199393,
            "range": "± 512",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1040475,
            "range": "± 2447",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1029423,
            "range": "± 9261",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5946607,
            "range": "± 37512",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6795282,
            "range": "± 29803",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 173521788,
            "range": "± 9959790",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 214154032,
            "range": "± 14299122",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5622815,
            "range": "± 48241",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6748239,
            "range": "± 33431",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 169385735,
            "range": "± 12084469",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 225421603,
            "range": "± 1422615",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 186693,
            "range": "± 233",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 212346,
            "range": "± 1931",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 969547,
            "range": "± 2601",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1084583,
            "range": "± 9753",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 26632578,
            "range": "± 474291",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 30327366,
            "range": "± 369608",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 936391281,
            "range": "± 2981111",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1109044680,
            "range": "± 7778235",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7136916,
            "range": "± 25835",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7860429,
            "range": "± 24995",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 108128355,
            "range": "± 431560",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 119282541,
            "range": "± 640485",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "wcrichto@cs.stanford.edu",
            "name": "Will Crichton",
            "username": "willcrichton"
          },
          "committer": {
            "email": "wcrichto@cs.stanford.edu",
            "name": "Will Crichton",
            "username": "willcrichton"
          },
          "distinct": true,
          "id": "eba2701ef36b924f117f4e669fbb2eaffa308eb5",
          "message": "Bump to 0.5.23",
          "timestamp": "2022-05-24T20:10:43-07:00",
          "tree_id": "31b0e39ef6db85499cd1605de02ec1a72183ff53",
          "url": "https://github.com/willcrichton/flowistry/commit/eba2701ef36b924f117f4e669fbb2eaffa308eb5"
        },
        "date": 1653449105369,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 181666,
            "range": "± 377",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 196872,
            "range": "± 1243",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 998689,
            "range": "± 3939",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1054455,
            "range": "± 3477",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5888391,
            "range": "± 57518",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6775773,
            "range": "± 25102",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 192209019,
            "range": "± 2396932",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 233292415,
            "range": "± 3179640",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5639396,
            "range": "± 13896",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6762322,
            "range": "± 10999",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 188910090,
            "range": "± 3368251",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 245757040,
            "range": "± 9187810",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 185951,
            "range": "± 172",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 208557,
            "range": "± 433",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 951473,
            "range": "± 2008",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1062893,
            "range": "± 2077",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28697028,
            "range": "± 859560",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 31243582,
            "range": "± 840825",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 949269133,
            "range": "± 1476054",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1140715776,
            "range": "± 4007105",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6947722,
            "range": "± 14625",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7666129,
            "range": "± 13461",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 111324221,
            "range": "± 563748",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 121343503,
            "range": "± 719298",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}