window.BENCHMARK_DATA = {
  "lastUpdate": 1669476555792,
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
          "id": "10d911810b7ed2dc5d887af4b3c5cf3b681b66da",
          "message": "Use lifetimes in type instead of accessible fields for doing alias analysis of opaque types",
          "timestamp": "2022-05-28T10:24:32-07:00",
          "tree_id": "66270f1258b5092f47422a3bcb2b902aa29446a5",
          "url": "https://github.com/willcrichton/flowistry/commit/10d911810b7ed2dc5d887af4b3c5cf3b681b66da"
        },
        "date": 1653759765669,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 215144,
            "range": "± 7879",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 235973,
            "range": "± 8252",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1176700,
            "range": "± 59201",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1322489,
            "range": "± 90847",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 7349186,
            "range": "± 949382",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7662046,
            "range": "± 470751",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 236981980,
            "range": "± 15466055",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 344138487,
            "range": "± 22347175",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6719573,
            "range": "± 448229",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7940896,
            "range": "± 364685",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 233080987,
            "range": "± 6501851",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 359245148,
            "range": "± 19178317",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 198803,
            "range": "± 8905",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 231373,
            "range": "± 11887",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1080004,
            "range": "± 82005",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1205484,
            "range": "± 82588",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 34229558,
            "range": "± 1327735",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 38530162,
            "range": "± 1965682",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1124966750,
            "range": "± 17575525",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1474262182,
            "range": "± 43141434",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7915021,
            "range": "± 397196",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8454038,
            "range": "± 352899",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 121990996,
            "range": "± 4206946",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 133847168,
            "range": "± 4259041",
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
          "id": "611b7972de10eb2ea38b945e3eda5402d34594fa",
          "message": "Fix IgnoreMut regression",
          "timestamp": "2022-05-28T11:03:26-07:00",
          "tree_id": "6113943ecf324c7e5e081917fc03cc73f3e87fa4",
          "url": "https://github.com/willcrichton/flowistry/commit/611b7972de10eb2ea38b945e3eda5402d34594fa"
        },
        "date": 1653761953273,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 208539,
            "range": "± 8071",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 225486,
            "range": "± 7696",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1181521,
            "range": "± 35013",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1236424,
            "range": "± 33763",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6591305,
            "range": "± 149631",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7493060,
            "range": "± 204734",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 211923599,
            "range": "± 7711592",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 254796668,
            "range": "± 18458241",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6278985,
            "range": "± 204328",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7670730,
            "range": "± 227614",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 207180074,
            "range": "± 13076884",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 272579106,
            "range": "± 14229224",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 204188,
            "range": "± 7373",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 235872,
            "range": "± 9381",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1035007,
            "range": "± 33381",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1155587,
            "range": "± 39453",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 31378335,
            "range": "± 1933542",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 34540992,
            "range": "± 1341085",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1051541657,
            "range": "± 12082957",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1248402087,
            "range": "± 14765849",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7629711,
            "range": "± 235089",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8250191,
            "range": "± 269625",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 123696694,
            "range": "± 4501649",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 130249327,
            "range": "± 3294628",
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
          "id": "36eeb4e4d4469e4febb63369c41c0327158c131e",
          "message": "Remove Body reference from Spanner to enable caching",
          "timestamp": "2022-06-02T12:19:53-07:00",
          "tree_id": "a080abd39590ca10ae70076019f5512755b0efda",
          "url": "https://github.com/willcrichton/flowistry/commit/36eeb4e4d4469e4febb63369c41c0327158c131e"
        },
        "date": 1654198602414,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 222793,
            "range": "± 1919",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 238915,
            "range": "± 3037",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1208568,
            "range": "± 62907",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1224277,
            "range": "± 9989",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6917469,
            "range": "± 108866",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7934730,
            "range": "± 99702",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 214552780,
            "range": "± 13470218",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 263843686,
            "range": "± 15989306",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6942029,
            "range": "± 103164",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 8282825,
            "range": "± 128073",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 217445510,
            "range": "± 20743612",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 292371120,
            "range": "± 6019631",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 224348,
            "range": "± 5852",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 248150,
            "range": "± 509",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1137434,
            "range": "± 38199",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1227014,
            "range": "± 8412",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 34863333,
            "range": "± 795625",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 38923946,
            "range": "± 514448",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1073323731,
            "range": "± 5689796",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1281180820,
            "range": "± 5782246",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 8261401,
            "range": "± 421051",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8932900,
            "range": "± 133285",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 130233356,
            "range": "± 4298071",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 139605814,
            "range": "± 6836967",
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
          "id": "7f6c42073075eac002238a04fb1b2552acccea1b",
          "message": "Bump to 0.5.24",
          "timestamp": "2022-06-02T14:27:33-07:00",
          "tree_id": "bb113a0ce428e4d58acd22fc06f70b3281ebd7f1",
          "url": "https://github.com/willcrichton/flowistry/commit/7f6c42073075eac002238a04fb1b2552acccea1b"
        },
        "date": 1654206125287,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 188092,
            "range": "± 230",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 198747,
            "range": "± 560",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 981260,
            "range": "± 13687",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1041987,
            "range": "± 3173",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5820248,
            "range": "± 43998",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6737885,
            "range": "± 39970",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 190473116,
            "range": "± 395626",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 232427927,
            "range": "± 1177716",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5651690,
            "range": "± 60713",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6873898,
            "range": "± 118998",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 189006323,
            "range": "± 690128",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 247931994,
            "range": "± 7862612",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 180748,
            "range": "± 162",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 201812,
            "range": "± 2492",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 924942,
            "range": "± 3607",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1027744,
            "range": "± 4682",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28497777,
            "range": "± 711403",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 32635777,
            "range": "± 833693",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 947500440,
            "range": "± 2897802",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1126296731,
            "range": "± 6739889",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6845602,
            "range": "± 25911",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7570660,
            "range": "± 28071",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 106494588,
            "range": "± 517095",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 117116435,
            "range": "± 555987",
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
          "id": "1f469a4cce9f2e240ad14b5145669a8011d4e6b2",
          "message": "Fix precision regression",
          "timestamp": "2022-06-03T12:57:47-07:00",
          "tree_id": "d5707b586fded7a2a06635d65995216dc2b38626",
          "url": "https://github.com/willcrichton/flowistry/commit/1f469a4cce9f2e240ad14b5145669a8011d4e6b2"
        },
        "date": 1654287242745,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 228231,
            "range": "± 17573",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 230973,
            "range": "± 4183",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1133721,
            "range": "± 17277",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1254492,
            "range": "± 96312",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6908166,
            "range": "± 282045",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 8050285,
            "range": "± 368747",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 219076999,
            "range": "± 3839291",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 264317640,
            "range": "± 14980347",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6572739,
            "range": "± 94877",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7966828,
            "range": "± 127194",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 212670851,
            "range": "± 5057470",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 282758089,
            "range": "± 16665353",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 210317,
            "range": "± 3935",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 245692,
            "range": "± 6041",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1101416,
            "range": "± 18586",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1226742,
            "range": "± 17634",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 34015956,
            "range": "± 918352",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 38613406,
            "range": "± 653985",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1069450340,
            "range": "± 17227358",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1281290598,
            "range": "± 7912032",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 8116518,
            "range": "± 113341",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8618607,
            "range": "± 158361",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 125040241,
            "range": "± 1986353",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 139868724,
            "range": "± 3915599",
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
          "id": "0df180324b8ae4297e0dd6a1667165c544f7f80e",
          "message": "Improve documentation",
          "timestamp": "2022-06-04T18:02:12-07:00",
          "tree_id": "fd34ae1d15f778dda0c5df16a7ce37e3baa87acd",
          "url": "https://github.com/willcrichton/flowistry/commit/0df180324b8ae4297e0dd6a1667165c544f7f80e"
        },
        "date": 1654391942107,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 225170,
            "range": "± 7170",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 243612,
            "range": "± 15278",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1234772,
            "range": "± 100096",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1246791,
            "range": "± 59445",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 7013050,
            "range": "± 278936",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 8143380,
            "range": "± 288292",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 223599347,
            "range": "± 21085135",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 272075515,
            "range": "± 13383840",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6790886,
            "range": "± 383494",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 8241792,
            "range": "± 232890",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 219269336,
            "range": "± 13419919",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 295457245,
            "range": "± 12825674",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 220489,
            "range": "± 8529",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 244152,
            "range": "± 10891",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1134601,
            "range": "± 42724",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1267418,
            "range": "± 39668",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 35300632,
            "range": "± 1826626",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 39230574,
            "range": "± 1858047",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1111789748,
            "range": "± 12169785",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1324286405,
            "range": "± 19347818",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7923542,
            "range": "± 315164",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8854880,
            "range": "± 467350",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 129241279,
            "range": "± 3607861",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 145722550,
            "range": "± 4467975",
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
          "id": "f323b5d227e89982316995343c07eef1e82e3530",
          "message": "Bump to 0.5.25",
          "timestamp": "2022-06-05T12:34:13-07:00",
          "tree_id": "f11efb9819ed4a9aff7e01fbb897ec9ff5fa676a",
          "url": "https://github.com/willcrichton/flowistry/commit/f323b5d227e89982316995343c07eef1e82e3530"
        },
        "date": 1654458486122,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 186066,
            "range": "± 214",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 198413,
            "range": "± 518",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 890103,
            "range": "± 3757",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 912757,
            "range": "± 2558",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5796556,
            "range": "± 14012",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 5958188,
            "range": "± 12188",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 187754092,
            "range": "± 439061",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 210996690,
            "range": "± 1312286",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 4992833,
            "range": "± 8870",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6031904,
            "range": "± 23341",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 186190514,
            "range": "± 4135584",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 224653791,
            "range": "± 3919643",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 165427,
            "range": "± 244",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 194254,
            "range": "± 528",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 911482,
            "range": "± 3930",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 902809,
            "range": "± 1365",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28392182,
            "range": "± 752686",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 27892653,
            "range": "± 701433",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 870147289,
            "range": "± 2176547",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1033148816,
            "range": "± 3401569",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 5895154,
            "range": "± 10978",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 6528587,
            "range": "± 7947",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 97118888,
            "range": "± 1135207",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 105819534,
            "range": "± 616018",
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
          "id": "8cc43829baf11eb64c943804ddf948179232f551",
          "message": "Ignore higher-order region variables, fixes #54.",
          "timestamp": "2022-06-06T15:27:16-07:00",
          "tree_id": "806756000bee56ca3412d82c46bd4259bc3e2d19",
          "url": "https://github.com/willcrichton/flowistry/commit/8cc43829baf11eb64c943804ddf948179232f551"
        },
        "date": 1654555376269,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 189785,
            "range": "± 1268",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 197585,
            "range": "± 1817",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 979480,
            "range": "± 2192",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1032996,
            "range": "± 2467",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5845744,
            "range": "± 28020",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6707927,
            "range": "± 22259",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 190336046,
            "range": "± 403977",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 230842165,
            "range": "± 993420",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5669672,
            "range": "± 12100",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6824848,
            "range": "± 19822",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 188559485,
            "range": "± 551697",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 246568530,
            "range": "± 1968878",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 179223,
            "range": "± 198",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 199521,
            "range": "± 129",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 916929,
            "range": "± 515",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1025365,
            "range": "± 1240",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28474347,
            "range": "± 992378",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 31255804,
            "range": "± 863318",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 942819223,
            "range": "± 1060792",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1126542048,
            "range": "± 1760774",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6703202,
            "range": "± 12617",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7416485,
            "range": "± 11893",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 106283504,
            "range": "± 397071",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 117422034,
            "range": "± 516688",
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
          "id": "c5b80acf3683f9093a915fb17bb8ebe01eb388d9",
          "message": "Bump to 0.5.26",
          "timestamp": "2022-06-06T16:25:24-07:00",
          "tree_id": "207e15698c8382a10c136cd1a711b19206011fed",
          "url": "https://github.com/willcrichton/flowistry/commit/c5b80acf3683f9093a915fb17bb8ebe01eb388d9"
        },
        "date": 1654558837234,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 185365,
            "range": "± 377",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 197926,
            "range": "± 794",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1017914,
            "range": "± 1212",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1034771,
            "range": "± 4160",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5833195,
            "range": "± 18439",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6738357,
            "range": "± 23357",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 214618316,
            "range": "± 592292",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 255439443,
            "range": "± 7794299",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5683118,
            "range": "± 8878",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6805295,
            "range": "± 18058",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 213594739,
            "range": "± 19307154",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 269554984,
            "range": "± 7391543",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 182738,
            "range": "± 314",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 205780,
            "range": "± 110",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 929748,
            "range": "± 1204",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1046502,
            "range": "± 1763",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 32148477,
            "range": "± 600096",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 35662086,
            "range": "± 673624",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1062629935,
            "range": "± 2971194",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1252185422,
            "range": "± 4479713",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6718522,
            "range": "± 22766",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7447459,
            "range": "± 20409",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 111912378,
            "range": "± 494597",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 122881774,
            "range": "± 829183",
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
          "id": "afdb397c989693b263f4c933ea8119671519e278",
          "message": "Fix remaining commands",
          "timestamp": "2022-06-21T09:37:41-07:00",
          "tree_id": "f20cb6ce25871e92023640f479c035f030c11c2e",
          "url": "https://github.com/willcrichton/flowistry/commit/afdb397c989693b263f4c933ea8119671519e278"
        },
        "date": 1655830631795,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 260891,
            "range": "± 13561",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 269895,
            "range": "± 11322",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1412448,
            "range": "± 56024",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1446273,
            "range": "± 62742",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 8378661,
            "range": "± 467757",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 9724076,
            "range": "± 466225",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 287344774,
            "range": "± 15386906",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 366614530,
            "range": "± 14180183",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 8028930,
            "range": "± 384579",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 9422077,
            "range": "± 381981",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 288556391,
            "range": "± 7119595",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 390951662,
            "range": "± 7177928",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 237169,
            "range": "± 10139",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 270994,
            "range": "± 14465",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1242626,
            "range": "± 49961",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1416587,
            "range": "± 34643",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 45646105,
            "range": "± 929630",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 50093025,
            "range": "± 3059526",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1398352731,
            "range": "± 19238962",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1642161723,
            "range": "± 45359500",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 9029477,
            "range": "± 430507",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 9881679,
            "range": "± 363329",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 141043228,
            "range": "± 3983704",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 153262527,
            "range": "± 4537830",
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
          "id": "6364b3b843b7faa6432124c7edf50b3875b888ca",
          "message": "Don't spawn a subshell to execute commands. Fixes #55",
          "timestamp": "2022-06-21T09:45:27-07:00",
          "tree_id": "4f5981218d5d6c74d3989863876a83e2a5a2eb62",
          "url": "https://github.com/willcrichton/flowistry/commit/6364b3b843b7faa6432124c7edf50b3875b888ca"
        },
        "date": 1655830865051,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 184657,
            "range": "± 1442",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 196477,
            "range": "± 1528",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1011526,
            "range": "± 7642",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1024853,
            "range": "± 3001",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5835232,
            "range": "± 21644",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6717493,
            "range": "± 25954",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 218511285,
            "range": "± 734209",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 261089087,
            "range": "± 1569131",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5664531,
            "range": "± 30172",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6799375,
            "range": "± 19290",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 218219086,
            "range": "± 16290976",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 277065750,
            "range": "± 1710577",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 183056,
            "range": "± 1322",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 203600,
            "range": "± 449",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 917393,
            "range": "± 3923",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1022024,
            "range": "± 4133",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28866040,
            "range": "± 839695",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 31858364,
            "range": "± 955494",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1075029068,
            "range": "± 3718114",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1258774608,
            "range": "± 6035136",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6646586,
            "range": "± 12163",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7286225,
            "range": "± 16385",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 108085452,
            "range": "± 819161",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 120362098,
            "range": "± 1057077",
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
          "id": "9d2003f818dcfe078f5fd204df93cc325d9b816d",
          "message": "Bump to 0.5.27",
          "timestamp": "2022-06-21T09:46:32-07:00",
          "tree_id": "ffa289a5ab3195b11add9b8a1aa276a16e9a8c1d",
          "url": "https://github.com/willcrichton/flowistry/commit/9d2003f818dcfe078f5fd204df93cc325d9b816d"
        },
        "date": 1655831100006,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 230879,
            "range": "± 10246",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 249959,
            "range": "± 11410",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1208901,
            "range": "± 44076",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1288706,
            "range": "± 44506",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 7210805,
            "range": "± 421435",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 8417081,
            "range": "± 488891",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 250917203,
            "range": "± 7652264",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 334931817,
            "range": "± 10603718",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 7028787,
            "range": "± 275711",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 8485613,
            "range": "± 293080",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 251639986,
            "range": "± 18774990",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 364254506,
            "range": "± 11294114",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 218484,
            "range": "± 6707",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 251778,
            "range": "± 9711",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1147719,
            "range": "± 39824",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1284829,
            "range": "± 77969",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 38991648,
            "range": "± 1778233",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 42596204,
            "range": "± 2078150",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1228297942,
            "range": "± 19775066",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1629199040,
            "range": "± 41726440",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 8352492,
            "range": "± 501816",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 9025996,
            "range": "± 391518",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 129971787,
            "range": "± 4134391",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 139922719,
            "range": "± 3612310",
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
          "id": "8235c491b6861353327468ddfb9c6d92595f96f7",
          "message": "Disable npm tests while network is being flaky",
          "timestamp": "2022-06-26T16:30:00-07:00",
          "tree_id": "8a5e4c9822a3ad7d6bcc2def195edbd1490d688e",
          "url": "https://github.com/willcrichton/flowistry/commit/8235c491b6861353327468ddfb9c6d92595f96f7"
        },
        "date": 1656287074857,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 191744,
            "range": "± 249",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 202756,
            "range": "± 363",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 990989,
            "range": "± 8155",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1034095,
            "range": "± 1032",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5824180,
            "range": "± 53780",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6680913,
            "range": "± 63582",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 172699720,
            "range": "± 542616",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 211716890,
            "range": "± 611061",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5701795,
            "range": "± 62263",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6740794,
            "range": "± 65700",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 169589651,
            "range": "± 449404",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 225153472,
            "range": "± 10297679",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 178499,
            "range": "± 655",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 201336,
            "range": "± 2670",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 943007,
            "range": "± 2314",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1060302,
            "range": "± 3143",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 25600799,
            "range": "± 569570",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 28891919,
            "range": "± 645136",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 922333976,
            "range": "± 2697569",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1089115039,
            "range": "± 2587236",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6777225,
            "range": "± 48994",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7521801,
            "range": "± 59463",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 104848249,
            "range": "± 496627",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 116053805,
            "range": "± 386466",
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
          "id": "3b60ee35eb569f9e67203c742f031db4c2f16034",
          "message": "Fix component installation",
          "timestamp": "2022-06-29T14:55:36-07:00",
          "tree_id": "e950b2b6cf2e242e5a02eee2fd84df0fd4372fbc",
          "url": "https://github.com/willcrichton/flowistry/commit/3b60ee35eb569f9e67203c742f031db4c2f16034"
        },
        "date": 1656540616324,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 184508,
            "range": "± 304",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 196040,
            "range": "± 154",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1013303,
            "range": "± 7553",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1015958,
            "range": "± 3557",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5883596,
            "range": "± 113258",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6741722,
            "range": "± 109738",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 191403216,
            "range": "± 1249200",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 233367469,
            "range": "± 7686370",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5702508,
            "range": "± 7995",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6810448,
            "range": "± 17717",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 189241154,
            "range": "± 13980266",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 246012004,
            "range": "± 12274211",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 184254,
            "range": "± 966",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 208451,
            "range": "± 201",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 911186,
            "range": "± 708",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1020367,
            "range": "± 1972",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28771736,
            "range": "± 929331",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 32032465,
            "range": "± 732654",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 954738467,
            "range": "± 4806386",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1131517322,
            "range": "± 5980519",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6572722,
            "range": "± 9522",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7287002,
            "range": "± 16926",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 106535163,
            "range": "± 1471195",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 116808802,
            "range": "± 639508",
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
          "id": "3760e0f3c9af1b9aed705fbda199cf1468d45a42",
          "message": "Remove unnecessary warning silence, fixes #56",
          "timestamp": "2022-09-13T17:15:12-07:00",
          "tree_id": "7735282dbc42481b7813795af9ca88bcce2d08d5",
          "url": "https://github.com/willcrichton/flowistry/commit/3760e0f3c9af1b9aed705fbda199cf1468d45a42"
        },
        "date": 1663115329508,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 186589,
            "range": "± 4557",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 199419,
            "range": "± 417",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1026651,
            "range": "± 6780",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1026087,
            "range": "± 1468",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5749250,
            "range": "± 42424",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6590086,
            "range": "± 37651",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 169418849,
            "range": "± 14091810",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 208931400,
            "range": "± 13466277",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5597830,
            "range": "± 40058",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6672234,
            "range": "± 29107",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 167338860,
            "range": "± 13532405",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 222699444,
            "range": "± 1897544",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 196083,
            "range": "± 6004",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 214391,
            "range": "± 9107",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 929834,
            "range": "± 3835",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1036805,
            "range": "± 7803",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 25449630,
            "range": "± 821583",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 28096798,
            "range": "± 859040",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 905100459,
            "range": "± 1870197",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1075319102,
            "range": "± 2072257",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6918727,
            "range": "± 68132",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7683003,
            "range": "± 51485",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 106763543,
            "range": "± 445576",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 117479664,
            "range": "± 1096439",
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
          "id": "cc5cefa1393395d0c4b47d65bd2e88666d966728",
          "message": "Update to nightly-2022-09-12",
          "timestamp": "2022-09-13T17:31:43-07:00",
          "tree_id": "f387a45e536fa33edb0f700c9f6f3fc091579008",
          "url": "https://github.com/willcrichton/flowistry/commit/cc5cefa1393395d0c4b47d65bd2e88666d966728"
        },
        "date": 1663116301281,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 179524,
            "range": "± 210",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 191749,
            "range": "± 1729",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1023320,
            "range": "± 5332",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1077163,
            "range": "± 2724",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5475287,
            "range": "± 25301",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6387773,
            "range": "± 66062",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 173251447,
            "range": "± 11439641",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 213415366,
            "range": "± 1442167",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5405187,
            "range": "± 19358",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6492816,
            "range": "± 34898",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 171553060,
            "range": "± 7282355",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 230848411,
            "range": "± 1622886",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 180235,
            "range": "± 1759",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 210724,
            "range": "± 241",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 885817,
            "range": "± 2264",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 998402,
            "range": "± 2060",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28316046,
            "range": "± 555175",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 31391320,
            "range": "± 648681",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 856044889,
            "range": "± 2040924",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1035523737,
            "range": "± 2789940",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6533936,
            "range": "± 30953",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7298090,
            "range": "± 30909",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 103422720,
            "range": "± 551354",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 114135821,
            "range": "± 795535",
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
          "id": "9f92ee481dc284df9c5ce206465cb98bf13e4275",
          "message": "Bump to 0.5.28",
          "timestamp": "2022-09-13T18:25:47-07:00",
          "tree_id": "f4b99247361113d17c1554417905806f11692359",
          "url": "https://github.com/willcrichton/flowistry/commit/9f92ee481dc284df9c5ce206465cb98bf13e4275"
        },
        "date": 1663119642670,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 209853,
            "range": "± 2927",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 225466,
            "range": "± 2565",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1151264,
            "range": "± 10468",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1203390,
            "range": "± 13887",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6629928,
            "range": "± 288276",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7638418,
            "range": "± 106763",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 191057589,
            "range": "± 14820446",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 257421738,
            "range": "± 3930468",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6390811,
            "range": "± 73100",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7681540,
            "range": "± 98735",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 188528562,
            "range": "± 14747644",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 276323928,
            "range": "± 20798876",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 207419,
            "range": "± 2967",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 236397,
            "range": "± 2955",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1065282,
            "range": "± 11499",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1193965,
            "range": "± 12320",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 32930187,
            "range": "± 725833",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 37122339,
            "range": "± 745865",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 945568983,
            "range": "± 5046128",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1155837161,
            "range": "± 7034428",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7890072,
            "range": "± 100861",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8684240,
            "range": "± 76482",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 123192416,
            "range": "± 2170826",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 135550860,
            "range": "± 1350832",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8296dc7b23c9bfe810e59644676a59b7aa72e269",
          "message": "Update README.md",
          "timestamp": "2022-09-14T18:01:41-07:00",
          "tree_id": "0c4ca62f8c4dc0f05a55c1bb17e0aae05eb803c8",
          "url": "https://github.com/willcrichton/flowistry/commit/8296dc7b23c9bfe810e59644676a59b7aa72e269"
        },
        "date": 1663204651753,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 214599,
            "range": "± 18997",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 225900,
            "range": "± 14531",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1261406,
            "range": "± 153362",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1190565,
            "range": "± 58724",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6433399,
            "range": "± 487571",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7304710,
            "range": "± 515295",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 209481807,
            "range": "± 7915637",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 294548361,
            "range": "± 9986224",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6089112,
            "range": "± 523429",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7401601,
            "range": "± 539488",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 221839739,
            "range": "± 13639268",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 331631567,
            "range": "± 10566146",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 222564,
            "range": "± 11484",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 248119,
            "range": "± 25503",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1115352,
            "range": "± 68643",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1230078,
            "range": "± 89468",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 38193448,
            "range": "± 1680286",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 40387020,
            "range": "± 2867553",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1025187660,
            "range": "± 39777068",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1402003554,
            "range": "± 42881591",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7058579,
            "range": "± 581986",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7810354,
            "range": "± 452650",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 111154533,
            "range": "± 5361563",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 123103435,
            "range": "± 6521385",
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
          "id": "35b0130f1e44b227ac7261f5ddb524c6233ff910",
          "message": "Fix example",
          "timestamp": "2022-09-28T13:51:02-04:00",
          "tree_id": "6487b1cf32c331516b3926a0c3163da4bd1115e2",
          "url": "https://github.com/willcrichton/flowistry/commit/35b0130f1e44b227ac7261f5ddb524c6233ff910"
        },
        "date": 1664388280915,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 177033,
            "range": "± 274",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 190640,
            "range": "± 494",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 969046,
            "range": "± 11914",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1026771,
            "range": "± 1741",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5541643,
            "range": "± 70308",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6418132,
            "range": "± 22526",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 167897701,
            "range": "± 5137518",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 212113214,
            "range": "± 5931017",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5342442,
            "range": "± 20394",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6498850,
            "range": "± 23197",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 166433515,
            "range": "± 6412266",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 224901308,
            "range": "± 2047038",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 182309,
            "range": "± 138",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 208076,
            "range": "± 822",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 914333,
            "range": "± 687",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1016150,
            "range": "± 1305",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28342806,
            "range": "± 611666",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 32352832,
            "range": "± 565147",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 832084922,
            "range": "± 1375901",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1015170579,
            "range": "± 2123277",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6490822,
            "range": "± 35224",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7216256,
            "range": "± 20470",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 104197706,
            "range": "± 585505",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 115991067,
            "range": "± 899574",
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
          "id": "39df755e7ba36c356e57d62ad62d2773c5508b1f",
          "message": "Add example of computing direct dependencies",
          "timestamp": "2022-09-28T14:00:50-04:00",
          "tree_id": "f89febed8daa66631b242282874393f9d860b6a7",
          "url": "https://github.com/willcrichton/flowistry/commit/39df755e7ba36c356e57d62ad62d2773c5508b1f"
        },
        "date": 1664388837649,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 181674,
            "range": "± 1005",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 194884,
            "range": "± 482",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1005154,
            "range": "± 8487",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1043720,
            "range": "± 16994",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5597062,
            "range": "± 98265",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6413049,
            "range": "± 249483",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 156218389,
            "range": "± 12199290",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 197435642,
            "range": "± 9406700",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5499799,
            "range": "± 118577",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6623936,
            "range": "± 162090",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 151876649,
            "range": "± 12271773",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 214371411,
            "range": "± 2461120",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 176439,
            "range": "± 955",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 201719,
            "range": "± 2328",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 908835,
            "range": "± 3299",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1018625,
            "range": "± 4453",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28359292,
            "range": "± 693527",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 31912188,
            "range": "± 700536",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 768953075,
            "range": "± 3388618",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 934247578,
            "range": "± 2510732",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6465034,
            "range": "± 16628",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7186214,
            "range": "± 25539",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 98321178,
            "range": "± 582171",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 108669681,
            "range": "± 409614",
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
          "id": "de7c68fdb49ed9975c7446340d4f605019a35bdb",
          "message": "Fix issue where main.rs is not part of project",
          "timestamp": "2022-10-15T16:28:29-04:00",
          "tree_id": "c7ee4f07fe135e879843ec8a5c98e514535c44df",
          "url": "https://github.com/willcrichton/flowistry/commit/de7c68fdb49ed9975c7446340d4f605019a35bdb"
        },
        "date": 1665866526329,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 177101,
            "range": "± 548",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 190148,
            "range": "± 684",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 974576,
            "range": "± 7318",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1015323,
            "range": "± 1700",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5496163,
            "range": "± 20367",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6378792,
            "range": "± 24486",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 168656669,
            "range": "± 16854236",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 209877449,
            "range": "± 784931",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5370886,
            "range": "± 37307",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6509973,
            "range": "± 14777",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 168599359,
            "range": "± 432382",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 223546651,
            "range": "± 1003257",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 178389,
            "range": "± 405",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 202803,
            "range": "± 159",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 898213,
            "range": "± 1297",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1004411,
            "range": "± 1648",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 27923587,
            "range": "± 499775",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 31094858,
            "range": "± 513408",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 840699605,
            "range": "± 1091033",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1015501849,
            "range": "± 3398172",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6534583,
            "range": "± 28581",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7245320,
            "range": "± 14286",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 103278640,
            "range": "± 515396",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 113241832,
            "range": "± 588389",
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
          "id": "3de1426301e8fca30cc9d70f0f5980a9efcdf641",
          "message": "Bump to 0.5.29",
          "timestamp": "2022-10-15T16:28:50-04:00",
          "tree_id": "4c927a12a800c760afe8a4fb63213b860eda6c60",
          "url": "https://github.com/willcrichton/flowistry/commit/3de1426301e8fca30cc9d70f0f5980a9efcdf641"
        },
        "date": 1665866584492,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 177675,
            "range": "± 254",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 190963,
            "range": "± 1276",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1006687,
            "range": "± 9934",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1008592,
            "range": "± 12393",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5550250,
            "range": "± 22735",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6403915,
            "range": "± 51160",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 191747251,
            "range": "± 4629313",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 236769000,
            "range": "± 3151409",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5394040,
            "range": "± 35138",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6523898,
            "range": "± 50238",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 191131857,
            "range": "± 11962882",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 248980273,
            "range": "± 9208419",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 175183,
            "range": "± 832",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 203290,
            "range": "± 776",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 897532,
            "range": "± 4451",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1005015,
            "range": "± 1415",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 29903571,
            "range": "± 913214",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 33538179,
            "range": "± 1057966",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 953926310,
            "range": "± 2891461",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1133452187,
            "range": "± 6676551",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6504215,
            "range": "± 48883",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7202003,
            "range": "± 46292",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 108899385,
            "range": "± 1235609",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 119676779,
            "range": "± 1019322",
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
          "id": "b972b71b6a2e54d7906576bd08f6e5b4b9b5a9a5",
          "message": "Make Flowistry less annoying when it can't find a file",
          "timestamp": "2022-10-15T17:08:33-04:00",
          "tree_id": "017683671a8a992f1933abc6fa0963c6813ff29e",
          "url": "https://github.com/willcrichton/flowistry/commit/b972b71b6a2e54d7906576bd08f6e5b4b9b5a9a5"
        },
        "date": 1665868907469,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 181160,
            "range": "± 584",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 194434,
            "range": "± 246",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 993416,
            "range": "± 745",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1043649,
            "range": "± 1889",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5492394,
            "range": "± 47586",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6344015,
            "range": "± 50010",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 151748163,
            "range": "± 10345204",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 191855331,
            "range": "± 1261740",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5317058,
            "range": "± 35706",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6408587,
            "range": "± 43942",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 151429373,
            "range": "± 7124932",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 207239604,
            "range": "± 8020441",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 173805,
            "range": "± 1405",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 195576,
            "range": "± 1459",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 905965,
            "range": "± 1563",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1009754,
            "range": "± 3108",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 25431324,
            "range": "± 648522",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 28567159,
            "range": "± 635084",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 770324747,
            "range": "± 2349322",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 939049372,
            "range": "± 3076762",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6458153,
            "range": "± 9784",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7176232,
            "range": "± 12490",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 99756641,
            "range": "± 454218",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 110256210,
            "range": "± 410360",
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
          "id": "7083ba316cfbe1d3489be477d8ebef14381a8d81",
          "message": "Bump to 0.5.30",
          "timestamp": "2022-10-15T17:11:04-04:00",
          "tree_id": "a6ba4eaca890314e02b7c526d5f7182e659ce734",
          "url": "https://github.com/willcrichton/flowistry/commit/7083ba316cfbe1d3489be477d8ebef14381a8d81"
        },
        "date": 1665869137135,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 178166,
            "range": "± 137",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 191029,
            "range": "± 320",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 973494,
            "range": "± 2140",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 972222,
            "range": "± 918",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5501732,
            "range": "± 21161",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6383200,
            "range": "± 33724",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 199581966,
            "range": "± 12592359",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 241765820,
            "range": "± 1658758",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5386385,
            "range": "± 49425",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6484357,
            "range": "± 57978",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 197982256,
            "range": "± 9154178",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 257646683,
            "range": "± 7429815",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 183305,
            "range": "± 291",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 207105,
            "range": "± 198",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 896162,
            "range": "± 1192",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 991860,
            "range": "± 894",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 30195361,
            "range": "± 876481",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 33978819,
            "range": "± 945205",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 966062067,
            "range": "± 4679912",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1148267789,
            "range": "± 3894370",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6482613,
            "range": "± 8986",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7184848,
            "range": "± 16622",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 109101088,
            "range": "± 874077",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 121231554,
            "range": "± 1033917",
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
          "id": "7ec6fade4f1e169d607e93c2094861bc831899c2",
          "message": "Add NO_SIMPLIFY flag to avoid breaking Polonius, add interior_paths function",
          "timestamp": "2022-11-05T19:26:48-07:00",
          "tree_id": "d63cc9400349030c1aac133acef8a508ee24d508",
          "url": "https://github.com/willcrichton/flowistry/commit/7ec6fade4f1e169d607e93c2094861bc831899c2"
        },
        "date": 1667702441390,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 181693,
            "range": "± 281",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 195769,
            "range": "± 319",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1015039,
            "range": "± 8573",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1067063,
            "range": "± 2254",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5586996,
            "range": "± 44271",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6414353,
            "range": "± 26532",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 154710003,
            "range": "± 3365636",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 194142295,
            "range": "± 1501512",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5459991,
            "range": "± 35093",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6493754,
            "range": "± 31807",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 152473169,
            "range": "± 5561375",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 208420633,
            "range": "± 17367508",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 176345,
            "range": "± 2112",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 199988,
            "range": "± 351",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 908465,
            "range": "± 1074",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1011390,
            "range": "± 3189",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 25540381,
            "range": "± 769778",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 29103748,
            "range": "± 837475",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 780168275,
            "range": "± 4358219",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 950839177,
            "range": "± 4456076",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6571029,
            "range": "± 10428",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7296904,
            "range": "± 44993",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 99592091,
            "range": "± 613913",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 110391264,
            "range": "± 847885",
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
          "id": "f4cedaccd7b56529155fba42a40a398216468480",
          "message": "Use new LocationOrArg type instead of hacky synthetic locations",
          "timestamp": "2022-11-07T08:24:24-08:00",
          "tree_id": "31c38ee8845ab94f213890b56781a400e928ea18",
          "url": "https://github.com/willcrichton/flowistry/commit/f4cedaccd7b56529155fba42a40a398216468480"
        },
        "date": 1667839312930,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 197528,
            "range": "± 12170",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 223013,
            "range": "± 11554",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1162630,
            "range": "± 80093",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1215071,
            "range": "± 71617",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6024961,
            "range": "± 351277",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7149329,
            "range": "± 351032",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 214293139,
            "range": "± 8827894",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 307167086,
            "range": "± 7028948",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5876597,
            "range": "± 354687",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7254757,
            "range": "± 367283",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 214227625,
            "range": "± 6836390",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 321355044,
            "range": "± 18037682",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 204212,
            "range": "± 15950",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 237530,
            "range": "± 16425",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1009599,
            "range": "± 57083",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1089194,
            "range": "± 112999",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 32339041,
            "range": "± 3748601",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 36932861,
            "range": "± 2496163",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1027409134,
            "range": "± 16414165",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1397064806,
            "range": "± 28518731",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6679171,
            "range": "± 390848",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7479019,
            "range": "± 287339",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 108871745,
            "range": "± 5815637",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 120320646,
            "range": "± 4911325",
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
          "id": "c7bafd4ab96e88a004004e117ed419c556697e7f",
          "message": "Update to nightly-2022-11-07, release v0.5.31.",
          "timestamp": "2022-11-08T16:58:26-08:00",
          "tree_id": "2486b3ac2aad544d8554acea2b1a94b78156dac4",
          "url": "https://github.com/willcrichton/flowistry/commit/c7bafd4ab96e88a004004e117ed419c556697e7f"
        },
        "date": 1667956363362,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 183029,
            "range": "± 1481",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 196578,
            "range": "± 657",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 948084,
            "range": "± 654",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 995904,
            "range": "± 1063",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5309081,
            "range": "± 24167",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6198615,
            "range": "± 26315",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 149585817,
            "range": "± 6258223",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 190002459,
            "range": "± 8837191",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5180501,
            "range": "± 42375",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6360821,
            "range": "± 67053",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 150123628,
            "range": "± 9164898",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 206983810,
            "range": "± 8818967",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 174438,
            "range": "± 169",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 197945,
            "range": "± 318",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 889806,
            "range": "± 17221",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1000964,
            "range": "± 2040",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 26165707,
            "range": "± 302289",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 29627865,
            "range": "± 247871",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 767394132,
            "range": "± 3892788",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 931025835,
            "range": "± 4938079",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6363646,
            "range": "± 21218",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7125313,
            "range": "± 29658",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 98126581,
            "range": "± 567317",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 109691691,
            "range": "± 931590",
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
          "id": "e29935c31b4f94e91d01ad9f3a1ba74ca8cf3c52",
          "message": "Add more documentation",
          "timestamp": "2022-11-16T09:04:13-08:00",
          "tree_id": "52dfe3e85670e93b116687db298356cc85e8b151",
          "url": "https://github.com/willcrichton/flowistry/commit/e29935c31b4f94e91d01ad9f3a1ba74ca8cf3c52"
        },
        "date": 1668619157709,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 179844,
            "range": "± 6139",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 172495,
            "range": "± 4263",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 823439,
            "range": "± 8605",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 865968,
            "range": "± 805",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 4840007,
            "range": "± 146475",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 5784118,
            "range": "± 239394",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 183314354,
            "range": "± 6423752",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 224202662,
            "range": "± 7041766",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5433099,
            "range": "± 146675",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6168807,
            "range": "± 307237",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 180318687,
            "range": "± 6427338",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 231642128,
            "range": "± 1340514",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 154345,
            "range": "± 382",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 181555,
            "range": "± 157",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 769029,
            "range": "± 1277",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 868193,
            "range": "± 2122",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 29587401,
            "range": "± 522956",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 32821611,
            "range": "± 400986",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 919784519,
            "range": "± 8470967",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1113481133,
            "range": "± 9293071",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 5770894,
            "range": "± 74238",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 6413630,
            "range": "± 129972",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 99804391,
            "range": "± 1562867",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 110759727,
            "range": "± 1604934",
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
          "id": "32a202c60faa56ded566a6137cd70e4234938bd6",
          "message": "Wow, look at all that documentation.",
          "timestamp": "2022-11-16T11:00:23-08:00",
          "tree_id": "c9643580a49db683ade1d10ca86b75350138fcf7",
          "url": "https://github.com/willcrichton/flowistry/commit/32a202c60faa56ded566a6137cd70e4234938bd6"
        },
        "date": 1668626150506,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 178449,
            "range": "± 651",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 192412,
            "range": "± 712",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 921934,
            "range": "± 2948",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 983671,
            "range": "± 562",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5341541,
            "range": "± 55331",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6291723,
            "range": "± 83920",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 189100786,
            "range": "± 13206222",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 250859930,
            "range": "± 2676805",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5189371,
            "range": "± 32420",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6392042,
            "range": "± 74625",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 190045017,
            "range": "± 2499759",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 277722491,
            "range": "± 4260278",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 175350,
            "range": "± 457",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 200344,
            "range": "± 793",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 864704,
            "range": "± 905",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 980897,
            "range": "± 903",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 31089358,
            "range": "± 526649",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 34824137,
            "range": "± 549455",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 953659116,
            "range": "± 2306990",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1153782193,
            "range": "± 4297665",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6284464,
            "range": "± 42409",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7016293,
            "range": "± 59458",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 109345756,
            "range": "± 1006447",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 121850978,
            "range": "± 1182634",
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
          "id": "d284f79488debb3a048daf4fe8fef1cf8e5f254d",
          "message": "Bump to v0.5.32",
          "timestamp": "2022-11-16T11:01:19-08:00",
          "tree_id": "b97cdfb8813c71dbe093dce4e2777330c3219f23",
          "url": "https://github.com/willcrichton/flowistry/commit/d284f79488debb3a048daf4fe8fef1cf8e5f254d"
        },
        "date": 1668626244820,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 211089,
            "range": "± 3219",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 225420,
            "range": "± 2688",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1166889,
            "range": "± 11091",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1220940,
            "range": "± 14559",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6297322,
            "range": "± 92794",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7377950,
            "range": "± 89681",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 182443092,
            "range": "± 2102948",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 234757135,
            "range": "± 1611286",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6085305,
            "range": "± 76044",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7496830,
            "range": "± 89177",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 181816912,
            "range": "± 3986019",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 251735582,
            "range": "± 3124523",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 205719,
            "range": "± 3829",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 240088,
            "range": "± 2393",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1035956,
            "range": "± 10654",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1155531,
            "range": "± 12557",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 30810443,
            "range": "± 928263",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 34320975,
            "range": "± 773006",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 923772966,
            "range": "± 2739501",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1150110664,
            "range": "± 4779859",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7421110,
            "range": "± 80958",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8316237,
            "range": "± 85645",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 116068053,
            "range": "± 1465169",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 130258389,
            "range": "± 1349955",
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
          "id": "9d893d2b78e980aee9422e06206d7b7235d55b9d",
          "message": "Update bibtex in flowistry sub-README",
          "timestamp": "2022-11-16T11:03:23-08:00",
          "tree_id": "7af2993c22db33b4af622d423a748eb20f83bcd2",
          "url": "https://github.com/willcrichton/flowistry/commit/9d893d2b78e980aee9422e06206d7b7235d55b9d"
        },
        "date": 1668626486995,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 222361,
            "range": "± 15960",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 241316,
            "range": "± 8479",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1296253,
            "range": "± 57381",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1321977,
            "range": "± 60751",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6742548,
            "range": "± 319425",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 8021244,
            "range": "± 472259",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 211185652,
            "range": "± 3663630",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 297496321,
            "range": "± 4648359",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6478760,
            "range": "± 210288",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 8097523,
            "range": "± 447120",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 214876371,
            "range": "± 11070071",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 324361043,
            "range": "± 4394884",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 213108,
            "range": "± 7068",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 247515,
            "range": "± 7477",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1101174,
            "range": "± 43102",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1250829,
            "range": "± 61471",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 37136880,
            "range": "± 1275803",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 41182504,
            "range": "± 1402367",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1050384419,
            "range": "± 11310657",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1481322571,
            "range": "± 17506635",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7847691,
            "range": "± 335321",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8727149,
            "range": "± 264652",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 122131192,
            "range": "± 4094198",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 135182914,
            "range": "± 4342453",
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
          "id": "68e7b8cec9fa8c0d9caccd553defea61343add3b",
          "message": "Search for workspace root relative to first opened Rust file. Fixes #62.",
          "timestamp": "2022-11-24T23:05:54-06:00",
          "tree_id": "6c80917073bc02e5b95eea4ffa703ac02a67a45d",
          "url": "https://github.com/willcrichton/flowistry/commit/68e7b8cec9fa8c0d9caccd553defea61343add3b"
        },
        "date": 1669353710519,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 180685,
            "range": "± 3979",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 196613,
            "range": "± 621",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 999734,
            "range": "± 9271",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1047220,
            "range": "± 1560",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5491127,
            "range": "± 59483",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6353968,
            "range": "± 76506",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 194970835,
            "range": "± 12343751",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 239562582,
            "range": "± 11179346",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5318055,
            "range": "± 37856",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6484417,
            "range": "± 82217",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 195252973,
            "range": "± 1171482",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 252980223,
            "range": "± 1246543",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 173074,
            "range": "± 501",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 201954,
            "range": "± 220",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 865839,
            "range": "± 1665",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 976763,
            "range": "± 23318",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 31622418,
            "range": "± 635745",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 35397096,
            "range": "± 1676462",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 963453638,
            "range": "± 4737511",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1107475359,
            "range": "± 5007761",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6456604,
            "range": "± 116818",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7234279,
            "range": "± 86071",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 110780406,
            "range": "± 1794098",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 119601012,
            "range": "± 1655088",
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
          "id": "77ca351f6ac511ef5293c03e7ad8dee2d624da0c",
          "message": "Revert CI runners to ubuntu-20.04",
          "timestamp": "2022-11-25T17:28:14-06:00",
          "tree_id": "01222e57ef79af02e1db346077988b680f763a16",
          "url": "https://github.com/willcrichton/flowistry/commit/77ca351f6ac511ef5293c03e7ad8dee2d624da0c"
        },
        "date": 1669419821721,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 203845,
            "range": "± 4510",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 216458,
            "range": "± 6617",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1050710,
            "range": "± 25905",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1115844,
            "range": "± 19550",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6043604,
            "range": "± 79834",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7121263,
            "range": "± 126786",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 175400364,
            "range": "± 11324722",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 223408964,
            "range": "± 11363555",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5899044,
            "range": "± 70566",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7197101,
            "range": "± 105853",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 174679259,
            "range": "± 1472393",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 238294150,
            "range": "± 2198557",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 195254,
            "range": "± 3869",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 227006,
            "range": "± 3122",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 977383,
            "range": "± 13186",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1108401,
            "range": "± 20756",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28253613,
            "range": "± 922815",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 32054318,
            "range": "± 820731",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 897948090,
            "range": "± 5844944",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1120613570,
            "range": "± 6070745",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7125582,
            "range": "± 104438",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7881563,
            "range": "± 127572",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 110719550,
            "range": "± 1496566",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 121790216,
            "range": "± 1585850",
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
          "id": "1d549e59cd4f28072f9c398699ab07ceef70a786",
          "message": "Use vscode.workspace.fs instead of node fs, apply formatting to JS",
          "timestamp": "2022-11-25T17:53:30-06:00",
          "tree_id": "cbb23d821119f2b39af760b35163aeaafb64c3a1",
          "url": "https://github.com/willcrichton/flowistry/commit/1d549e59cd4f28072f9c398699ab07ceef70a786"
        },
        "date": 1669421474560,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 226869,
            "range": "± 71533",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 245051,
            "range": "± 14781",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1221748,
            "range": "± 89859",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1270291,
            "range": "± 103089",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 7545009,
            "range": "± 694068",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 8686528,
            "range": "± 818199",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 237465069,
            "range": "± 8474483",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 312021799,
            "range": "± 8454911",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6949290,
            "range": "± 571557",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 8348060,
            "range": "± 645994",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 238123088,
            "range": "± 11437640",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 347875549,
            "range": "± 13599755",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 226431,
            "range": "± 17662",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 264020,
            "range": "± 19447",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1121778,
            "range": "± 63078",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1292837,
            "range": "± 114864",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 40258403,
            "range": "± 3250510",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 43301669,
            "range": "± 2776739",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 1144724650,
            "range": "± 35782968",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1405617834,
            "range": "± 33300623",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 8302621,
            "range": "± 461190",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 8886029,
            "range": "± 456409",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 121399037,
            "range": "± 4356963",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 141199001,
            "range": "± 6080461",
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
          "id": "23135abd376ebe7f3e129ed5e98337d61f2707c6",
          "message": "Add more logging to VSCode setup",
          "timestamp": "2022-11-25T18:35:05-06:00",
          "tree_id": "4bb84dd8fbe3831ec2ad61addd6c49142a9a441a",
          "url": "https://github.com/willcrichton/flowistry/commit/23135abd376ebe7f3e129ed5e98337d61f2707c6"
        },
        "date": 1669423778685,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 177894,
            "range": "± 614",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 188385,
            "range": "± 866",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 926786,
            "range": "± 510",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 974156,
            "range": "± 577",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5345079,
            "range": "± 39128",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6298433,
            "range": "± 66318",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 165158279,
            "range": "± 18368093",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 208079446,
            "range": "± 1736485",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5273349,
            "range": "± 71798",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6503491,
            "range": "± 116754",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 163206613,
            "range": "± 14572379",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 222211642,
            "range": "± 8108475",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 171046,
            "range": "± 443",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 200520,
            "range": "± 190",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 863469,
            "range": "± 1637",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 972369,
            "range": "± 533",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 28161340,
            "range": "± 379763",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 31027627,
            "range": "± 555872",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 824433953,
            "range": "± 3015225",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1019911300,
            "range": "± 5781891",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6306695,
            "range": "± 31750",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7027521,
            "range": "± 36994",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 99246848,
            "range": "± 598985",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 110648590,
            "range": "± 1114016",
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
          "id": "b6edacb61d12f52099d8e75bba6db011e5ea0fd9",
          "message": "Only open files, not directory",
          "timestamp": "2022-11-25T18:46:33-06:00",
          "tree_id": "a158a8ce54d5ef61bd90bbbf63fd10a22143e0a1",
          "url": "https://github.com/willcrichton/flowistry/commit/b6edacb61d12f52099d8e75bba6db011e5ea0fd9"
        },
        "date": 1669424542858,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 224022,
            "range": "± 16035",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 237190,
            "range": "± 23435",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 1241687,
            "range": "± 36756",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1370178,
            "range": "± 163278",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 6098044,
            "range": "± 138886",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 7197003,
            "range": "± 182158",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 184152439,
            "range": "± 18880576",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 234179365,
            "range": "± 14750640",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 6085026,
            "range": "± 152832",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 7629746,
            "range": "± 110714",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 182421577,
            "range": "± 13700886",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 250333275,
            "range": "± 14203022",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 201335,
            "range": "± 5469",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 225882,
            "range": "± 5835",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 1008154,
            "range": "± 28483",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 1129916,
            "range": "± 31449",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 30684989,
            "range": "± 1164585",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 33388250,
            "range": "± 1387114",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 925947806,
            "range": "± 8175739",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1147945466,
            "range": "± 9734195",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 7421526,
            "range": "± 439176",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7874939,
            "range": "± 185220",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 111830038,
            "range": "± 2831222",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 128819973,
            "range": "± 4837097",
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
          "id": "4346dadbd0dd1c74388e413b4bafb06eb2fd5335",
          "message": "Fix findWorkspaceRoot failing in CI",
          "timestamp": "2022-11-25T22:52:46-06:00",
          "tree_id": "1004442ab5cce835648b757422e6d3666d3d09a9",
          "url": "https://github.com/willcrichton/flowistry/commit/4346dadbd0dd1c74388e413b4bafb06eb2fd5335"
        },
        "date": 1669439209637,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 182783,
            "range": "± 904",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 195974,
            "range": "± 651",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 952968,
            "range": "± 6046",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 1000395,
            "range": "± 1005",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5200503,
            "range": "± 39727",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6077687,
            "range": "± 26771",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 146370829,
            "range": "± 10039407",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 186264974,
            "range": "± 2471991",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5085683,
            "range": "± 46525",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6225270,
            "range": "± 57482",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 144845434,
            "range": "± 6982938",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 201702567,
            "range": "± 2479120",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 169594,
            "range": "± 405",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 194138,
            "range": "± 525",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 870165,
            "range": "± 433",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 978121,
            "range": "± 974",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 23508032,
            "range": "± 730914",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 26782878,
            "range": "± 831939",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 741759648,
            "range": "± 2437456",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 898001178,
            "range": "± 3573348",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6279283,
            "range": "± 10305",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 6961276,
            "range": "± 8837",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 96027128,
            "range": "± 567262",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 106058176,
            "range": "± 814815",
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
          "id": "603b83dc67d0ec181ace235a61955df7147c1482",
          "message": "Bump to v0.5.33",
          "timestamp": "2022-11-26T09:14:50-06:00",
          "tree_id": "0c4e06d2c3ff2fef48c57a76fdd4a417e728b191",
          "url": "https://github.com/willcrichton/flowistry/commit/603b83dc67d0ec181ace235a61955df7147c1482"
        },
        "date": 1669476547839,
        "tool": "cargo",
        "benches": [
          {
            "name": "Locations (min)/Flow",
            "value": 176924,
            "range": "± 734",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (min)/Flow + Deps",
            "value": 189742,
            "range": "± 644",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow",
            "value": 976801,
            "range": "± 1920",
            "unit": "ns/iter"
          },
          {
            "name": "Locations (max)/Flow + Deps",
            "value": 970309,
            "range": "± 1452",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow",
            "value": 5308993,
            "range": "± 36172",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (min)/Flow + Deps",
            "value": 6237755,
            "range": "± 78714",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow",
            "value": 162858230,
            "range": "± 1697269",
            "unit": "ns/iter"
          },
          {
            "name": "Unique Lifetimes (max)/Flow + Deps",
            "value": 205604964,
            "range": "± 3162497",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow",
            "value": 5135925,
            "range": "± 24521",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (min)/Flow + Deps",
            "value": 6283020,
            "range": "± 58171",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow",
            "value": 161168248,
            "range": "± 683292",
            "unit": "ns/iter"
          },
          {
            "name": "Infoflow (max)/Flow + Deps",
            "value": 219626168,
            "range": "± 1194343",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow",
            "value": 168653,
            "range": "± 1670",
            "unit": "ns/iter"
          },
          {
            "name": "Places (min)/Flow + Deps",
            "value": 197564,
            "range": "± 187",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow",
            "value": 867995,
            "range": "± 2375",
            "unit": "ns/iter"
          },
          {
            "name": "Places (max)/Flow + Deps",
            "value": 991138,
            "range": "± 11665",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow",
            "value": 26391132,
            "range": "± 965545",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (min)/Flow + Deps",
            "value": 29640275,
            "range": "± 1092442",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow",
            "value": 817770431,
            "range": "± 4131022",
            "unit": "ns/iter"
          },
          {
            "name": "Same Lifetime (max)/Flow + Deps",
            "value": 1006839994,
            "range": "± 3959458",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow",
            "value": 6243696,
            "range": "± 16496",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (min)/Flow + Deps",
            "value": 7001950,
            "range": "± 61846",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow",
            "value": 102023491,
            "range": "± 1231159",
            "unit": "ns/iter"
          },
          {
            "name": "Nested Structs (max)/Flow + Deps",
            "value": 112326003,
            "range": "± 1207499",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}